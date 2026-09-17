//! Private, bounded, atomically replaced files under Fut's XDG state directory.

use std::{
    env,
    ffi::OsStr,
    fs,
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use uuid::Uuid;

/// `$XDG_STATE_HOME/fut/<file_name>`, or `~/.local/state/fut/<file_name>`.
pub(crate) fn path(file_name: &str) -> Result<PathBuf> {
    path_from(
        file_name,
        env::var_os("XDG_STATE_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
    )
}

fn path_from(
    file_name: &str,
    xdg_state_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Result<PathBuf> {
    if let Some(directory) = xdg_state_home.filter(|value| !value.is_empty()) {
        let directory = PathBuf::from(directory);
        if !directory.is_absolute() {
            bail!("XDG_STATE_HOME must be an absolute path when resolving Fut state");
        }
        return Ok(directory.join("fut").join(file_name));
    }
    let home = home
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .context("HOME must be set when XDG_STATE_HOME is not set")?;
    if !home.is_absolute() {
        bail!("HOME must be an absolute path when resolving Fut state");
    }
    Ok(home.join(".local/state/fut").join(file_name))
}

/// Exclusive advisory lock on `<file>.lock`, held until dropped.
pub(crate) struct Lock {
    _file: fs::File,
}

impl Lock {
    pub(crate) fn acquire(path: &Path) -> Result<Self> {
        let file = Self::open(path)?;
        loop {
            // SAFETY: `file` owns a valid descriptor and `LOCK_EX` has no pointer arguments.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
                break;
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error).with_context(|| format!("lock {}", path.display()));
            }
        }
        Ok(Self { _file: file })
    }

    pub(crate) async fn acquire_async(path: &Path) -> Result<Self> {
        let file = Self::open(path)?;
        loop {
            // SAFETY: `file` owns a valid descriptor and `flock` has no pointer arguments.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                return Ok(Self { _file: file });
            }
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::EWOULDBLOCK) {
                return Err(error).with_context(|| format!("lock {}", path.display()));
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    fn open(path: &Path) -> Result<fs::File> {
        prepare_parent(path)?;
        let lock_path = path.with_extension("lock");
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&lock_path)
            .with_context(|| format!("open lock {}", lock_path.display()))?;
        if !file
            .metadata()
            .with_context(|| format!("inspect lock {}", lock_path.display()))?
            .file_type()
            .is_file()
        {
            bail!("lock {} is not a regular file", lock_path.display());
        }
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .with_context(|| format!("secure lock {}", lock_path.display()))?;
        Ok(file)
    }
}

/// Reads a private regular file, or `None` when it does not exist yet.
/// Symlinks, other file types, permissions other than 0600, and files larger
/// than `max_bytes` are rejected rather than partially trusted.
pub(crate) fn read(path: &Path, max_bytes: u64) -> Result<Option<String>> {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("open {}", path.display())),
    };
    let metadata = file
        .metadata()
        .with_context(|| format!("inspect {}", path.display()))?;
    if !metadata.file_type().is_file() {
        bail!("{} is not a regular file", path.display());
    }
    if metadata.permissions().mode() & 0o7777 != 0o600 {
        bail!("{} must have permissions 0600", path.display());
    }
    if metadata.len() > max_bytes {
        bail!(
            "{} is {} bytes; maximum is {max_bytes}",
            path.display(),
            metadata.len()
        );
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("read {}", path.display()))?;
    if bytes.len() as u64 > max_bytes {
        bail!("{} exceeds the {max_bytes}-byte maximum", path.display());
    }
    String::from_utf8(bytes)
        .map(Some)
        .with_context(|| format!("{} is not UTF-8", path.display()))
}

/// Atomically replaces `path` with `contents` as a 0600 regular file.
pub(crate) fn write(path: &Path, contents: &str, max_bytes: u64) -> Result<()> {
    if contents.len() as u64 > max_bytes {
        bail!(
            "updated {} exceeds the {max_bytes}-byte maximum",
            path.display()
        );
    }
    prepare_parent(path)?;
    let parent = path.parent().expect("validated parent directory");
    let temporary_path = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        Uuid::new_v4()
    ));
    let result = (|| -> Result<()> {
        let mut temporary = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temporary_path)
            .with_context(|| format!("create {}", temporary_path.display()))?;
        temporary
            .set_permissions(fs::Permissions::from_mode(0o600))
            .with_context(|| format!("secure {}", temporary_path.display()))?;
        temporary
            .write_all(contents.as_bytes())
            .with_context(|| format!("write {}", temporary_path.display()))?;
        temporary
            .sync_all()
            .with_context(|| format!("sync {}", temporary_path.display()))?;
        fs::rename(&temporary_path, path)
            .with_context(|| format!("atomically replace {}", path.display()))?;
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .with_context(|| format!("sync directory {}", parent.display()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn prepare_parent(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .with_context(|| format!("{} must have a parent directory", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("create directory {}", parent.display()))?;
    let metadata = fs::symlink_metadata(parent)
        .with_context(|| format!("inspect directory {}", parent.display()))?;
    if !metadata.file_type().is_dir() {
        bail!("{} is not a directory", parent.display());
    }
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("secure directory {}", parent.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_prefers_valid_xdg_and_validates_fallback_home() {
        fn xdg(value: &'static str) -> Option<&'static OsStr> {
            Some(OsStr::new(value))
        }
        assert_eq!(
            path_from("machines.toml", xdg("/state"), None).unwrap(),
            Path::new("/state/fut/machines.toml")
        );
        assert_eq!(
            path_from("trusted-recipes.toml", None, xdg("/home/user")).unwrap(),
            Path::new("/home/user/.local/state/fut/trusted-recipes.toml")
        );
        assert_eq!(
            path_from("machines.toml", xdg(""), xdg("/home/user")).unwrap(),
            Path::new("/home/user/.local/state/fut/machines.toml")
        );
        assert!(path_from("machines.toml", xdg("relative"), None).is_err());
        assert!(path_from("machines.toml", None, xdg("relative")).is_err());
        assert!(path_from("machines.toml", None, None).is_err());
    }

    #[test]
    fn files_are_private_atomic_bounded_and_never_follow_symlinks() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("state/fut/example.toml");
        assert_eq!(read(&path, 64).unwrap(), None);

        write(&path, "a = 1\n", 64).unwrap();
        let parent = path.parent().unwrap();
        assert_eq!(
            fs::metadata(parent).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(read(&path, 64).unwrap().as_deref(), Some("a = 1\n"));
        assert_eq!(
            fs::read_dir(parent).unwrap().count(),
            1,
            "temporary files must not linger"
        );
        assert!(write(&path, &"x".repeat(65), 64).is_err());
        assert_eq!(read(&path, 64).unwrap().as_deref(), Some("a = 1\n"));

        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(format!("{:#}", read(&path, 64).unwrap_err()).contains("permissions 0600"));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(format!("{:#}", read(&path, 3).unwrap_err()).contains("maximum"));

        let elsewhere = temporary.path().join("elsewhere.toml");
        fs::write(&elsewhere, "b = 2\n").unwrap();
        fs::set_permissions(&elsewhere, fs::Permissions::from_mode(0o600)).unwrap();
        let link = temporary.path().join("state/fut/link.toml");
        std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
        assert!(read(&link, 64).is_err());
        assert!(
            write(&link, "c = 3\n", 64).is_ok(),
            "rename replaces the link itself"
        );
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_file());
        assert_eq!(fs::read_to_string(&elsewhere).unwrap(), "b = 2\n");

        let _lock = Lock::acquire(&path).unwrap();
        assert!(path.with_extension("lock").is_file());
    }
}
