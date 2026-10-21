//! Tokens and passwords in the Secret Service keyring (GNOME Keyring,
//! KWallet's Secret Service, KeePassXC, …) through `secret-tool` from
//! libsecret, never in a plain file.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Mutex;

/// Somewhere to keep secrets by key: the [`Keyring`], or [`Memory`] in
/// tests.
pub trait Secrets: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    /// `label` is what keyring managers show for the entry.
    fn set(&self, key: &str, label: &str, value: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

/// `secret-tool` with the attributes `service=<service> key=<key>`.
#[derive(Clone, Debug)]
pub struct Keyring {
    pub service: String,
    program: String,
}

impl Keyring {
    /// `service` names the app, e.g. "m3-calendar".
    pub fn new(service: impl Into<String>) -> Self {
        Self { service: service.into(), program: "secret-tool".into() }
    }

    /// Whether `secret-tool` is installed.
    pub fn available() -> bool {
        crate::sys::cmd::exists("secret-tool")
    }
}

impl Secrets for Keyring {
    fn get(&self, key: &str) -> Result<Option<String>> {
        if !crate::sys::cmd::exists(&self.program) {
            return Err(anyhow!("secret-tool isn't installed (it comes with libsecret)"));
        }
        match crate::sys::cmd::run(&self.program, &["lookup", "service", &self.service, "key", key]) {
            Ok(v) if v.is_empty() => Ok(None),
            Ok(v) => Ok(Some(v.trim_end_matches('\n').to_string())),
            // lookup exits 1 without output when there's no such secret.
            Err(e) if e.contains("exited with") => Ok(None),
            Err(e) => Err(anyhow!("keyring: {e}")),
        }
    }

    fn set(&self, key: &str, label: &str, value: &str) -> Result<()> {
        crate::sys::cmd::run_with_stdin(
            &self.program,
            &["store", &format!("--label={label}"), "service", &self.service, "key", key],
            value.as_bytes(),
        )
        .map(|_| ())
        .map_err(|e| anyhow!("couldn't save to the keyring: {e}"))
    }

    fn delete(&self, key: &str) -> Result<()> {
        let _ = crate::sys::cmd::run(&self.program, &["clear", "service", &self.service, "key", key]);
        Ok(())
    }
}

/// Secrets in memory, for tests.
#[derive(Default)]
pub struct Memory(Mutex<HashMap<String, String>>);

impl Secrets for Memory {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.0.lock().map_err(|_| anyhow!("poisoned"))?.get(key).cloned())
    }

    fn set(&self, key: &str, _label: &str, value: &str) -> Result<()> {
        self.0.lock().map_err(|_| anyhow!("poisoned"))?.insert(key.into(), value.into());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<()> {
        self.0.lock().map_err(|_| anyhow!("poisoned"))?.remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_round_trip() {
        let m = Memory::default();
        assert_eq!(m.get("a").unwrap(), None);
        m.set("a", "label", "secret").unwrap();
        assert_eq!(m.get("a").unwrap().as_deref(), Some("secret"));
        m.delete("a").unwrap();
        assert_eq!(m.get("a").unwrap(), None);
    }

    #[test]
    fn keyring_speaks_secret_tool() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        // A secret-tool that keeps one file per service/key and logs labels.
        let fake = dir.path().join("secret-tool");
        let script = format!(
            r#"#!/bin/sh
d='{}'; mkdir -p "$d"
case "$1" in
  lookup) [ -f "$d/$3-$5" ] || exit 1; cat "$d/$3-$5" ;;
  store) echo "$2" > "$d/label"; cat > "$d/$4-$6" ;;
  clear) rm -f "$d/$3-$5" ;;
esac
"#,
            store.display()
        );
        // Written by a child process: an executable this process had open
        // for writing could be inherited by a concurrent fork (ETXTBSY).
        let path = fake.to_string_lossy();
        crate::sys::cmd::run_with_stdin("sh", &["-c", r#"cat > "$0" && chmod 755 "$0""#, &path], script.as_bytes())
            .unwrap();
        let k = Keyring { service: "m3-test".into(), program: fake.to_string_lossy().into_owned() };
        assert_eq!(k.get("account:1").unwrap(), None);
        k.set("account:1", "Test: me", "refresh-token").unwrap();
        assert_eq!(k.get("account:1").unwrap().as_deref(), Some("refresh-token"));
        assert_eq!(std::fs::read_to_string(store.join("label")).unwrap().trim(), "--label=Test: me");
        assert!(store.join("m3-test-account:1").is_file(), "service and key are the attributes");
        k.delete("account:1").unwrap();
        assert_eq!(k.get("account:1").unwrap(), None);

        let missing = Keyring { service: "x".into(), program: "/nonexistent/secret-tool".into() };
        assert!(missing.get("k").is_err());
    }
}
