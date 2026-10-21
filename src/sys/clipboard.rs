//! The Wayland clipboard through `wl-copy`.

use std::io::Write;
use std::process::{Command, Stdio};

/// Put `bytes` on the clipboard as `mime` (e.g. "image/png").
pub fn copy_bytes(bytes: &[u8], mime: &str) -> Result<(), String> {
    if !super::cmd::exists("wl-copy") {
        return Err("wl-copy isn't installed (pacman -S wl-clipboard).".into());
    }
    copy_with("wl-copy", &["--type", mime], bytes)
}

/// Put text on the clipboard.
pub fn copy_text(text: &str) -> Result<(), String> {
    copy_bytes(text.as_bytes(), "text/plain;charset=utf-8")
}

/// wl-copy forks a child that serves the clipboard until something else is
/// copied. That child inherits stdout and stderr, so they go to /dev/null:
/// capturing them would wait for the child, i.e. forever. Only the parent's
/// exit is waited for.
fn copy_with(program: &str, args: &[&str], bytes: &[u8]) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{program}: {e}"))?;
    let written = child.stdin.take().ok_or_else(|| format!("{program}: no stdin"))?.write_all(bytes);
    // stdin is closed here, so the program sees the end of the data.
    let status = child.wait().map_err(|e| format!("{program}: {e}"))?;
    written.map_err(|e| format!("{program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed ({status})."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn returns_while_the_forked_server_lives_on() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("clip");
        // Like wl-copy: read the data, leave a child running, exit.
        let script = format!("cat > '{}'; (sleep 5 &); exit 0", out.display());
        let start = Instant::now();
        copy_with("sh", &["-c", &script], b"hello").unwrap();
        assert!(start.elapsed() < Duration::from_secs(3), "didn't wait for the child");
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "hello");
        assert!(copy_with("sh", &["-c", "cat >/dev/null; exit 3"], b"x").is_err());
        assert!(copy_with("definitely-not-a-real-program-m3s", &[], b"x").is_err());
    }
}
