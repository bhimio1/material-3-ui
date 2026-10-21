//! Saving files safely: writing through symlinks (stow/chezmoi dotfiles
//! stay links), atomic replacement when that keeps the file's identity,
//! `pkexec` for files the user can't write, and timestamped backups (with
//! the `backups` feature).

use std::fs;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// What a file looked like when it was last read or written, to notice
/// changes made by other programs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub mtime: Option<SystemTime>,
    pub len: u64,
}

pub fn stamp(path: &Path) -> Option<Stamp> {
    let m = fs::metadata(path).ok()?;
    Some(Stamp { mtime: m.modified().ok(), len: m.len() })
}

/// The real file behind a path: symlinks are followed (even dangling
/// ones), so saving writes through the link and the link stays.
pub fn resolve(path: &Path) -> PathBuf {
    if let Ok(real) = fs::canonicalize(path) {
        return real;
    }
    let mut p = path.to_path_buf();
    for _ in 0..32 {
        match fs::read_link(&p) {
            Ok(target) => {
                p = if target.is_absolute() { target } else { p.parent().unwrap_or(Path::new("/")).join(target) };
            }
            Err(_) => break,
        }
    }
    p
}

/// Whether the file can be written (or created, when it doesn't exist).
pub fn writable(path: &Path) -> bool {
    let target = resolve(path);
    match fs::metadata(&target) {
        Ok(_) => fs::OpenOptions::new().append(true).open(&target).is_ok(),
        Err(_) => target.ancestors().skip(1).find(|d| d.exists()).is_some_and(dir_writable),
    }
}

fn dir_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".m3-probe-{}", std::process::id()));
    match fs::File::create(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[derive(Debug)]
pub enum SaveError {
    /// Permission denied: offer to save as administrator ([`pkexec_write`]).
    NeedsRoot,
    Io(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::NeedsRoot => write!(f, "permission denied"),
            SaveError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SaveError {}

fn io_err(e: std::io::Error) -> SaveError {
    if e.kind() == ErrorKind::PermissionDenied {
        SaveError::NeedsRoot
    } else {
        SaveError::Io(e.to_string())
    }
}

/// Write `bytes` to `path`, through symlinks, creating missing folders. An
/// existing file is replaced atomically (temp file, fsync, rename, mode
/// kept) when that doesn't change its owner or break hard links; otherwise
/// it's rewritten in place. Returns the new [`Stamp`].
pub fn save(path: &Path, bytes: &[u8]) -> Result<Stamp, SaveError> {
    let target = resolve(path);
    let dir = target.parent().ok_or_else(|| SaveError::Io("no parent folder".into()))?;
    match fs::metadata(&target) {
        Ok(meta) => {
            let atomic = meta.nlink() == 1 && atomic_replace(&target, dir, &meta, bytes).is_ok();
            if !atomic {
                let mut f = fs::OpenOptions::new().write(true).truncate(true).open(&target).map_err(io_err)?;
                f.write_all(bytes).map_err(io_err)?;
                f.sync_all().map_err(io_err)?;
            }
        }
        Err(_) => {
            fs::create_dir_all(dir).map_err(io_err)?;
            let mut f = fs::File::create(&target).map_err(io_err)?;
            f.write_all(bytes).map_err(io_err)?;
            f.sync_all().map_err(io_err)?;
        }
    }
    stamp(&target).ok_or_else(|| SaveError::Io("the file vanished after saving".into()))
}

/// [`save`] for text, with errors as strings.
pub fn save_text(path: &Path, text: &str) -> Result<(), String> {
    save(path, text.as_bytes()).map(|_| ()).map_err(|e| match e {
        SaveError::NeedsRoot => format!("{}: permission denied", path.display()),
        SaveError::Io(e) => e,
    })
}

fn atomic_replace(target: &Path, dir: &Path, meta: &fs::Metadata, bytes: &[u8]) -> std::io::Result<()> {
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.m3-{}", std::process::id()));
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        // A file owned by someone else would change hands: don't.
        let made = f.metadata()?;
        if made.uid() != meta.uid() || made.gid() != meta.gid() {
            return Err(std::io::Error::other("owner differs"));
        }
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::set_permissions(&tmp, meta.permissions())?;
        fs::rename(&tmp, target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Write a file as root through `pkexec` (one password prompt). Symlinks
/// are followed, missing folders created, and the file is rewritten in
/// place so its owner and mode stay. With `keep_backup`, the previous
/// contents are kept next to it as `<name>.bak`.
pub fn pkexec_write(path: &Path, bytes: &[u8], keep_backup: bool) -> Result<(), String> {
    privileged_write(&["pkexec"], path, bytes, keep_backup)
}

/// The script behind [`pkexec_write`], run through `prefix` (tests use
/// `env` instead of `pkexec`).
fn privileged_write(prefix: &[&str], path: &Path, bytes: &[u8], keep_backup: bool) -> Result<(), String> {
    let target = resolve(path);
    let target = target.to_string_lossy();
    let script = r#"set -e; mkdir -p "$(dirname "$1")"; if [ "$2" = 1 ] && [ -f "$1" ]; then cp -p "$1" "$1.bak"; fi; cat > "$1""#;
    let mut args: Vec<&str> = prefix[1..].to_vec();
    args.extend(["sh", "-c", script, "material-3", &target, if keep_backup { "1" } else { "0" }]);
    crate::sys::cmd::run_with_stdin(prefix[0], &args, bytes).map(|_| ())
}

/// Timestamped copies of files before they're overwritten, in one folder,
/// newest `keep` per file. Names are the file's full path (`/` as `%`) and
/// the local time, e.g. `home%me%.config%kitty%kitty.conf@20261020-140312.123`.
#[cfg(feature = "backups")]
#[derive(Clone, Debug)]
pub struct Backups {
    pub dir: PathBuf,
    pub keep: usize,
}

#[cfg(feature = "backups")]
impl Backups {
    pub fn new(dir: impl Into<PathBuf>, keep: usize) -> Self {
        Self { dir: dir.into(), keep }
    }

    fn stem(path: &Path) -> String {
        path.to_string_lossy().trim_start_matches('/').replace('/', "%")
    }

    /// Copy the file behind `path` into the folder. Drops the oldest
    /// copies beyond `keep`.
    pub fn backup(&self, path: &Path) -> std::io::Result<PathBuf> {
        let target = resolve(path);
        fs::create_dir_all(&self.dir)?;
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S%.3f");
        let dest = self.dir.join(format!("{}@{stamp}", Self::stem(&target)));
        fs::copy(&target, &dest)?;
        for stale in self.list(path).iter().skip(self.keep.max(1)) {
            let _ = fs::remove_file(stale);
        }
        Ok(dest)
    }

    /// Backups of `path`, newest first.
    pub fn list(&self, path: &Path) -> Vec<PathBuf> {
        let prefix = format!("{}@", Self::stem(&resolve(path)));
        let mut found: Vec<PathBuf> = fs::read_dir(&self.dir)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&prefix)))
                    .collect()
            })
            .unwrap_or_default();
        found.sort();
        found.reverse();
        found
    }

    /// Save `bytes` to `path` with [`save`], backing up what was there.
    pub fn save(&self, path: &Path, bytes: &[u8]) -> Result<Stamp, SaveError> {
        if resolve(path).is_file() {
            self.backup(path).map_err(|e| SaveError::Io(format!("couldn't back up: {e}")))?;
        }
        save(path, bytes)
    }
}

/// "2026-10-20 14:03:12" from a backup's file name.
#[cfg(feature = "backups")]
pub fn backup_label(backup: &Path) -> String {
    let name = backup.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stamp = name.rsplit('@').next().unwrap_or_default();
    chrono::NaiveDateTime::parse_from_str(stamp, "%Y%m%d-%H%M%S%.3f")
        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|_| stamp.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn saves_through_symlinks_and_keeps_mode() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.conf");
        fs::write(&real, "old").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o640)).unwrap();
        let link = dir.path().join("link.conf");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let st = save(&link, b"new").unwrap();
        assert_eq!(st.len, 3);
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "link kept");
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
        assert_eq!(fs::metadata(&real).unwrap().permissions().mode() & 0o777, 0o640);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2, "no temp files left");
        assert_eq!(stamp(&link), Some(st));
    }

    #[test]
    fn hard_links_are_written_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        fs::write(&a, "1").unwrap();
        fs::hard_link(&a, &b).unwrap();
        save(&a, b"2").unwrap();
        assert_eq!(fs::read_to_string(&b).unwrap(), "2");
    }

    #[test]
    fn creates_new_files_and_folders() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub/dir/new.txt");
        assert!(writable(&p));
        save_text(&p, "hello").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "hello");
    }

    #[test]
    fn dangling_symlink_resolves_to_target() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("l");
        std::os::unix::fs::symlink("missing.txt", &link).unwrap();
        assert_eq!(resolve(&link), dir.path().join("missing.txt"));
        save(&link, b"x").unwrap();
        assert_eq!(fs::read_to_string(dir.path().join("missing.txt")).unwrap(), "x");
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    }

    #[test]
    fn read_only_files_need_root() {
        if std::process::Command::new("id").arg("-u").output().is_ok_and(|o| o.stdout.starts_with(b"0\n")) {
            return; // root can write anything
        }
        let dir = tempfile::tempdir().unwrap();
        let ro = dir.path().join("ro");
        fs::create_dir(&ro).unwrap();
        let file = ro.join("f");
        fs::write(&file, "1").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o444)).unwrap();
        fs::set_permissions(&ro, fs::Permissions::from_mode(0o555)).unwrap();
        assert!(!writable(&file));
        assert!(matches!(save(&file, b"2"), Err(SaveError::NeedsRoot)));
        fs::set_permissions(&ro, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn privileged_script_follows_links_and_keeps_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("pacman.conf");
        fs::write(&real, "old").unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        privileged_write(&["env"], &link, b"new", true).unwrap();
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
        assert_eq!(fs::read_to_string(dir.path().join("pacman.conf.bak")).unwrap(), "old");
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());

        let fresh = dir.path().join("a/b/c.conf");
        privileged_write(&["env"], &fresh, b"\x00bytes", false).unwrap();
        assert_eq!(fs::read(&fresh).unwrap(), b"\x00bytes");
        assert!(!dir.path().join("a/b/c.conf.bak").exists());
    }

    #[cfg(feature = "backups")]
    #[test]
    fn backups_are_listed_newest_first_and_pruned() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("kitty.conf");
        let backups = Backups::new(dir.path().join("backups"), 2);
        backups.save(&file, b"1").unwrap();
        assert!(backups.list(&file).is_empty(), "nothing to back up yet");
        for v in ["2", "3", "4"] {
            std::thread::sleep(std::time::Duration::from_millis(5));
            backups.save(&file, v.as_bytes()).unwrap();
        }
        let list = backups.list(&file);
        assert_eq!(list.len(), 2);
        assert_eq!(fs::read_to_string(&list[0]).unwrap(), "3");
        assert_eq!(fs::read_to_string(&list[1]).unwrap(), "2");
        assert_eq!(backup_label(&list[0]).len(), "2026-10-20 14:03:12".len());
        assert_eq!(backup_label(Path::new("x@garbage")), "garbage");
    }
}
