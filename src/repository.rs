use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::agent;
use crate::config::Config;
use crate::database;
use crate::error::BelayError;

const ENTRY_DIRECTORIES: &[&str] = &["goals", "plans", "decisions", "work", "reviews", "notes"];

const BELAY_GITIGNORE: &str = "state/\n*.sqlite-wal\n*.sqlite-shm\n";

#[derive(Debug, Clone)]
pub struct Repository {
    pub root: PathBuf,
    pub belay_dir: PathBuf,
    pub config: Config,
}

#[derive(Debug, Clone)]
pub struct InitOutcome {
    pub repository: Repository,
    pub already_initialized: bool,
}

impl Repository {
    pub fn database_path(&self) -> PathBuf {
        self.belay_dir.join(&self.config.storage.database)
    }

    pub fn entries_path(&self) -> PathBuf {
        self.belay_dir.join(&self.config.storage.entries)
    }

    pub fn evidence_path(&self) -> PathBuf {
        self.belay_dir.join("evidence")
    }
}

pub fn discover(start: &Path) -> Result<Repository, BelayError> {
    let start = directory_start(start);
    for candidate in start.ancestors() {
        let belay_dir = candidate.join(".belay");
        let config_path = belay_dir.join("config.toml");
        if path_exists(&config_path)? {
            require_directory(&belay_dir)?;
            require_regular_file(&config_path)?;
            let config = Config::load(&config_path)?;
            return Ok(Repository {
                root: candidate.to_path_buf(),
                belay_dir,
                config,
            });
        }
        if candidate.join(".jj").exists() || candidate.join(".git").exists() {
            return Err(BelayError::Uninitialized {
                root: candidate.to_path_buf(),
            });
        }
    }

    Err(BelayError::Uninitialized {
        root: start.to_path_buf(),
    })
}

pub fn initialize(start: &Path) -> Result<InitOutcome, BelayError> {
    let root = match discover(start) {
        Ok(repository) => repository.root,
        Err(BelayError::Uninitialized { .. }) => find_project_root(directory_start(start)),
        Err(error) => return Err(error),
    };
    let belay_dir = root.join(".belay");
    let config_path = belay_dir.join("config.toml");
    let already_initialized = path_exists(&config_path)?;

    ensure_directory(&belay_dir)?;
    if !already_initialized {
        let rendered = Config::default().render()?;
        write_if_missing(&config_path, rendered.as_bytes())?;
    } else {
        require_regular_file(&config_path)?;
    }

    let config = Config::load(&config_path)?;
    let repository = Repository {
        root,
        belay_dir,
        config,
    };

    ensure_layout(&repository)?;
    agent::refresh_generated_assets(&repository)?;
    database::initialize(&repository.database_path())?;

    Ok(InitOutcome {
        repository,
        already_initialized,
    })
}

fn ensure_layout(repository: &Repository) -> Result<(), BelayError> {
    ensure_managed_directory(&repository.belay_dir, &repository.config.storage.entries)?;
    for directory in ENTRY_DIRECTORIES {
        ensure_managed_directory(
            &repository.belay_dir,
            &repository.config.storage.entries.join(directory),
        )?;
    }

    let database_path = repository.database_path();
    let database_parent =
        repository
            .config
            .storage
            .database
            .parent()
            .ok_or_else(|| BelayError::Validation {
                message: format!(
                    "configured database path {} has no parent directory",
                    database_path.display()
                ),
            })?;
    ensure_managed_directory(&repository.belay_dir, database_parent)?;
    if path_exists(&database_path)? {
        require_regular_file(&database_path)?;
    }

    ensure_managed_directory(&repository.belay_dir, Path::new("agent/codex"))?;
    ensure_managed_directory(&repository.belay_dir, Path::new("agent/claude"))?;
    ensure_managed_directory(&repository.belay_dir, Path::new("evidence"))?;
    ensure_managed_directory(&repository.belay_dir, Path::new("evidence/records"))?;
    ensure_managed_directory(&repository.belay_dir, Path::new("lifecycle"))?;
    let lock = repository.belay_dir.join("lifecycle/writer.lock");
    if path_exists(&lock)? {
        require_regular_file(&lock)?;
    } else {
        write_if_missing(&lock, b"")?;
    }

    ensure_gitignore(&repository.belay_dir.join(".gitignore"))?;
    Ok(())
}

fn ensure_managed_directory(belay_dir: &Path, relative: &Path) -> Result<(), BelayError> {
    let mut current = belay_dir.to_path_buf();
    require_directory(&current)?;
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            return Err(BelayError::Validation {
                message: format!(
                    "managed directory {} must stay within {}",
                    relative.display(),
                    belay_dir.display()
                ),
            });
        };
        current.push(component);
        ensure_directory(&current)?;
    }
    Ok(())
}

fn ensure_directory(path: &Path) -> Result<(), BelayError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(BelayError::Validation {
            message: format!(
                "managed path {} must not be a symbolic link",
                path.display()
            ),
        }),
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(BelayError::Validation {
            message: format!("managed path {} must be a directory", path.display()),
        }),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path)
                .map_err(|source| BelayError::io("create directory", path, source))?;
            require_directory(path)
        }
        Err(source) => Err(BelayError::io("inspect", path, source)),
    }
}

fn write_if_missing(path: &Path, contents: &[u8]) -> Result<(), BelayError> {
    if path_exists(path)? {
        return require_regular_file(path);
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| BelayError::io("create", path, source))?;
    file.write_all(contents)
        .map_err(|source| BelayError::io("write", path, source))
}

fn ensure_gitignore(path: &Path) -> Result<(), BelayError> {
    if !path_exists(path)? {
        return write_if_missing(path, BELAY_GITIGNORE.as_bytes());
    }
    require_regular_file(path)?;

    let existing =
        fs::read_to_string(path).map_err(|source| BelayError::io("read", path, source))?;
    let mut missing = Vec::new();
    for required in BELAY_GITIGNORE.lines() {
        if !existing.lines().any(|line| line.trim() == required) {
            missing.push(required);
        }
    }
    if missing.is_empty() {
        return Ok(());
    }

    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|source| BelayError::io("open", path, source))?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        file.write_all(b"\n")
            .map_err(|source| BelayError::io("write", path, source))?;
    }
    for line in missing {
        writeln!(file, "{line}").map_err(|source| BelayError::io("write", path, source))?;
    }
    Ok(())
}

fn path_exists(path: &Path) -> Result<bool, BelayError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(BelayError::io("inspect", path, source)),
    }
}

fn require_directory(path: &Path) -> Result<(), BelayError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|source| BelayError::io("inspect", path, source))?;
    if metadata.file_type().is_symlink() {
        return Err(BelayError::Validation {
            message: format!(
                "managed path {} must not be a symbolic link",
                path.display()
            ),
        });
    }
    if !metadata.is_dir() {
        return Err(BelayError::Validation {
            message: format!("managed path {} must be a directory", path.display()),
        });
    }
    Ok(())
}

fn require_regular_file(path: &Path) -> Result<(), BelayError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|source| BelayError::io("inspect", path, source))?;
    if metadata.file_type().is_symlink() {
        return Err(BelayError::Validation {
            message: format!(
                "managed path {} must not be a symbolic link",
                path.display()
            ),
        });
    }
    if !metadata.is_file() {
        return Err(BelayError::Validation {
            message: format!("managed path {} must be a regular file", path.display()),
        });
    }
    Ok(())
}

fn directory_start(path: &Path) -> &Path {
    if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    }
}

fn find_project_root(start: &Path) -> PathBuf {
    start
        .ancestors()
        .find(|candidate| candidate.join(".jj").exists() || candidate.join(".git").exists())
        .unwrap_or(start)
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rusqlite::Connection;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn init_is_idempotent_and_discovers_from_nested_directories() {
        let temporary = tempdir().expect("create temp directory");
        let root = temporary.path();
        fs::create_dir(root.join(".git")).expect("create repository marker");
        fs::write(root.join("AGENTS.md"), "existing instructions\n").expect("write AGENTS.md");
        let nested = root.join("src/deep");
        fs::create_dir_all(&nested).expect("create nested path");

        let first = initialize(&nested).expect("initialize repository");
        assert!(!first.already_initialized);
        assert_eq!(first.repository.root, root);

        let expected_paths = [
            ".belay/config.toml",
            ".belay/.gitignore",
            ".belay/state/belay.sqlite",
            ".belay/entries/goals",
            ".belay/entries/plans",
            ".belay/entries/decisions",
            ".belay/entries/work",
            ".belay/entries/reviews",
            ".belay/entries/notes",
            ".belay/evidence",
            ".belay/agent/AGENTS.md.snippet",
            ".belay/agent/claude/SKILL.md",
            ".belay/agent/codex/SKILL.md",
        ];
        for path in expected_paths {
            assert!(root.join(path).exists(), "missing {path}");
        }
        assert_eq!(
            fs::read_to_string(root.join("AGENTS.md")).expect("read AGENTS.md"),
            "existing instructions\n"
        );

        let config_before =
            fs::read_to_string(root.join(".belay/config.toml")).expect("read config");
        let second = initialize(&nested).expect("repeat initialization");
        assert!(second.already_initialized);
        assert_eq!(
            fs::read_to_string(root.join(".belay/config.toml")).expect("read config again"),
            config_before
        );

        let discovered = discover(&nested).expect("discover initialized repository");
        assert_eq!(discovered.root, root);

        let connection =
            Connection::open(discovered.database_path()).expect("open initialized database");
        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("count migrations");
        assert_eq!(migration_count, crate::database::LATEST_SCHEMA_VERSION);
    }

    #[test]
    fn repeated_init_repairs_required_gitignore_entries() {
        let temporary = tempdir().expect("create temp directory");
        let root = temporary.path();
        fs::create_dir(root.join(".git")).expect("create repository marker");
        initialize(root).expect("initialize repository");
        fs::write(root.join(".belay/.gitignore"), "custom-entry\n").expect("replace gitignore");

        initialize(root).expect("repair initialization");
        let gitignore = fs::read_to_string(root.join(".belay/.gitignore")).expect("read gitignore");
        assert!(gitignore.contains("custom-entry"));
        for required in BELAY_GITIGNORE.lines() {
            assert!(gitignore.lines().any(|line| line.trim() == required));
        }
    }

    #[cfg(unix)]
    #[test]
    fn init_rejects_symlinked_belay_directory() {
        use std::os::unix::fs::symlink;

        let temporary = tempdir().expect("create temp directory");
        let root = temporary.path().join("repository");
        let external = temporary.path().join("external");
        fs::create_dir_all(root.join(".git")).expect("create repository marker");
        fs::create_dir(&external).expect("create external directory");
        symlink(&external, root.join(".belay")).expect("create symlink");

        let error = initialize(&root).expect_err("symlinked .belay must be rejected");
        assert!(matches!(error, BelayError::Validation { .. }));
        assert!(!external.join("config.toml").exists());
    }

    #[cfg(unix)]
    #[test]
    fn init_rejects_symlinked_managed_subdirectories() {
        use std::os::unix::fs::symlink;

        for managed_path in ["state", "entries"] {
            let temporary = tempdir().expect("create temp directory");
            let root = temporary.path().join("repository");
            let external = temporary.path().join("external");
            fs::create_dir_all(root.join(".git")).expect("create repository marker");
            fs::create_dir_all(root.join(".belay")).expect("create belay directory");
            fs::write(
                root.join(".belay/config.toml"),
                Config::default().render().expect("render config"),
            )
            .expect("write config");
            fs::create_dir(&external).expect("create external directory");
            symlink(&external, root.join(".belay").join(managed_path)).expect("create symlink");

            let error = initialize(&root).expect_err("managed symlink must be rejected");
            assert!(matches!(error, BelayError::Validation { .. }));
            assert!(
                fs::read_dir(&external)
                    .expect("read external directory")
                    .next()
                    .is_none()
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn init_rejects_symlinked_managed_files() {
        use std::os::unix::fs::symlink;

        let temporary = tempdir().expect("create temp directory");
        let root = temporary.path().join("repository");
        let external_config = temporary.path().join("external-config.toml");
        fs::create_dir_all(root.join(".git")).expect("create repository marker");
        fs::create_dir(root.join(".belay")).expect("create belay directory");
        fs::write(
            &external_config,
            Config::default().render().expect("render config"),
        )
        .expect("write external config");
        symlink(&external_config, root.join(".belay/config.toml")).expect("symlink config");

        let error = initialize(&root).expect_err("symlinked config must be rejected");
        assert!(matches!(error, BelayError::Validation { .. }));

        fs::remove_file(root.join(".belay/config.toml")).expect("remove config symlink");
        fs::write(
            root.join(".belay/config.toml"),
            Config::default().render().expect("render config"),
        )
        .expect("write config");
        fs::create_dir(root.join(".belay/state")).expect("create state directory");
        let external_database = temporary.path().join("external.sqlite");
        fs::write(&external_database, b"not a database").expect("write external database");
        symlink(&external_database, root.join(".belay/state/belay.sqlite"))
            .expect("symlink database");

        let error = initialize(&root).expect_err("symlinked database must be rejected");
        assert!(matches!(error, BelayError::Validation { .. }));
        assert_eq!(
            fs::read(&external_database).expect("read external database"),
            b"not a database"
        );
    }

    #[test]
    fn discovery_does_not_cross_a_nested_repository_boundary() {
        let temporary = tempdir().expect("create temp directory");
        let parent = temporary.path();
        fs::create_dir(parent.join(".git")).expect("create parent marker");
        initialize(parent).expect("initialize parent");

        let nested = parent.join("nested");
        fs::create_dir_all(nested.join(".git")).expect("create nested marker");
        let error = discover(&nested).expect_err("nested repository is not initialized");

        match error {
            BelayError::Uninitialized { root } => assert_eq!(root, nested),
            other => panic!("unexpected error: {other}"),
        }
    }
}
