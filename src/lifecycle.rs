//! Shared durability, identity and writer coordination for lifecycle storage.
use crate::{BelayError, repository::Repository, store};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::rc::{Rc, Weak};

thread_local! { static LOCKS: RefCell<BTreeMap<PathBuf, Weak<LockInner>>> = const { RefCell::new(BTreeMap::new()) }; }
struct LockInner {
    _file: fs::File,
}
pub struct WriterGuard {
    _inner: Rc<LockInner>,
}

pub fn writer_lock(repository: &Repository) -> Result<WriterGuard, BelayError> {
    let root = fs::canonicalize(&repository.belay_dir)
        .map_err(|e| BelayError::io("resolve managed root", &repository.belay_dir, e))?;
    if let Some(inner) = LOCKS.with(|locks| locks.borrow().get(&root).and_then(Weak::upgrade)) {
        return Ok(WriterGuard { _inner: inner });
    }
    ensure_directory(repository, Path::new("lifecycle"))?;
    let path = root.join("lifecycle/writer.lock");
    #[cfg(unix)]
    let file = {
        use rustix::fs::{FlockOperation, Mode, OFlags};
        let fd = rustix::fs::open(
            &path,
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|e| BelayError::io("open lifecycle lock", &path, std::io::Error::from(e)))?;
        let file = fs::File::from(fd);
        if !file
            .metadata()
            .map_err(|e| BelayError::io("inspect lifecycle lock", &path, e))?
            .is_file()
        {
            return invalid("lifecycle lock must be a regular file");
        }
        rustix::fs::flock(&file, FlockOperation::LockExclusive).map_err(|e| {
            BelayError::io("lock lifecycle writers", &path, std::io::Error::from(e))
        })?;
        file
    };
    #[cfg(not(unix))]
    return invalid("lifecycle writer locking is unsupported on this platform");
    #[cfg(unix)]
    {
        let inner = Rc::new(LockInner { _file: file });
        LOCKS.with(|locks| {
            locks.borrow_mut().insert(root, Rc::downgrade(&inner));
        });
        Ok(WriterGuard { _inner: inner })
    }
}

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn random_id() -> Result<String, BelayError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| BelayError::Capability {
        message: format!("secure identity entropy unavailable: {e}"),
    })?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

pub fn ensure_directory(repository: &Repository, relative: &Path) -> Result<(), BelayError> {
    validate_relative(relative)?;
    let mut current = repository.belay_dir.clone();
    for component in relative.components() {
        current.push(component);
        match fs::create_dir(&current) {
            Ok(()) => sync_directory(current.parent().expect("parent"))?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(BelayError::io("create lifecycle directory", &current, e)),
        }
        let m = fs::symlink_metadata(&current)
            .map_err(|e| BelayError::io("inspect lifecycle directory", &current, e))?;
        if m.file_type().is_symlink() || !m.is_dir() {
            return invalid(format!(
                "managed directory {} must be a real directory",
                current.display()
            ));
        }
    }
    Ok(())
}
pub fn sync_directory(path: &Path) -> Result<(), BelayError> {
    #[cfg(unix)]
    {
        use rustix::fs::{Mode, OFlags};
        let fd = rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| BelayError::io("open directory for sync", path, std::io::Error::from(e)))?;
        fs::File::from(fd)
            .sync_all()
            .map_err(|e| BelayError::io("sync directory", path, e))
    }
    #[cfg(not(unix))]
    {
        invalid("durable directory sync unsupported on this platform")
    }
}
pub fn validate_relative(path: &Path) -> Result<(), BelayError> {
    if path.as_os_str().is_empty() || !path.components().all(|p| matches!(p, Component::Normal(_)))
    {
        return invalid(format!(
            "managed lifecycle path {} must be relative without traversal",
            path.display()
        ));
    }
    Ok(())
}
pub fn read_original(repository: &Repository, path: &Path) -> Result<Vec<u8>, BelayError> {
    match store::read_loose_managed_file(repository, path) {
        Ok(raw) => Ok(raw.into_bytes()),
        Err(error) => {
            if matches!(&error, BelayError::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound)
            {
                if let Some(raw) = crate::pack::original_at(repository, path)? {
                    return Ok(raw.into_bytes());
                }
            }
            Err(error)
        }
    }
}
pub fn ensure_v2(repository: &Repository) -> Result<(), BelayError> {
    let _guard = writer_lock(repository)?;
    let path = repository.belay_dir.join("config.toml");
    let raw = store::read_loose_managed_file(repository, &path)?;
    let mut config = crate::config::Config::load(&path)?;
    if config.schema_version == 2 {
        return Ok(());
    }
    config.schema_version = 2;
    // Config has no Markdown semantic hash; this replacement is a raw CAS.
    replace_raw(
        repository,
        &path,
        config.render()?.as_bytes(),
        &hash(raw.as_bytes()),
    )
}
pub fn replace_raw(
    repository: &Repository,
    path: &Path,
    bytes: &[u8],
    expected: &str,
) -> Result<(), BelayError> {
    let old = store::read_loose_managed_file(repository, path)?;
    if hash(old.as_bytes()) != expected {
        return Err(BelayError::Conflict {
            message: format!("{} changed since snapshot", path.display()),
        });
    }
    let parent = path.parent().ok_or_else(|| BelayError::Validation {
        message: "missing parent".into(),
    })?;
    let temp = parent.join(format!(".tmp-{}", random_id()?));
    store::write_new_file(repository, &temp, bytes)?;
    let current = store::read_loose_managed_file(repository, path)?;
    if hash(current.as_bytes()) != expected {
        let _ = fs::remove_file(&temp);
        return Err(BelayError::Conflict {
            message: format!("{} changed during replacement", path.display()),
        });
    }
    fs::rename(&temp, path)
        .map_err(|e| BelayError::io("publish lifecycle replacement", path, e))?;
    sync_directory(parent)
}
pub fn invalid<T>(message: impl Into<String>) -> Result<T, BelayError> {
    Err(BelayError::Validation {
        message: message.into(),
    })
}
