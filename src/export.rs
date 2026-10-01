#[cfg(unix)]
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::fd::OwnedFd;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{TransactionBehavior, params};
#[cfg(unix)]
use rustix::fs::{AtFlags, Mode, OFlags};
use serde::Serialize;

use crate::entry::{Entry, EntryStatus, EntryType, MARKDOWN_SCHEMA_VERSION};
use crate::error::BelayError;
use crate::repository::Repository;
use crate::store;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
    Json,
    Ndjson,
}

#[derive(Debug, Clone, Default)]
pub struct ExportFilter {
    pub entry_type: Option<EntryType>,
    pub status: Option<EntryStatus>,
    pub tag: Option<String>,
    pub display_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct ExportDocument<'a> {
    schema_version: u32,
    entries: &'a [ExportEntry],
}

#[derive(Debug, Serialize)]
struct ExportEntry {
    schema_version: u32,
    id: String,
    #[serde(rename = "type")]
    entry_type: EntryType,
    title: String,
    status: EntryStatus,
    created_at: String,
    updated_at: String,
    revision: u32,
    tags: Vec<String>,
    links: Vec<crate::entry::EntryLink>,
    metadata: std::collections::BTreeMap<String, crate::entry::MetadataValue>,
    body: String,
    source_path: String,
}

pub fn write(
    repository: &Repository,
    format: ExportFormat,
    output: &Path,
    filter: &ExportFilter,
) -> Result<usize, BelayError> {
    validate_filter(filter)?;
    crate::evidence::validate_index_sources(repository)?;
    let output = absolute_path(&repository.root, output);
    let entries = load_entries(repository, filter)?;
    let rendered = match format {
        ExportFormat::Markdown => render_markdown(&entries),
        ExportFormat::Json => serde_json::to_vec_pretty(&ExportDocument {
            schema_version: 1,
            entries: &entries,
        })
        .map_err(serialization_error)?,
        ExportFormat::Ndjson => render_ndjson(&entries)?,
    };
    write_atomic(repository, &output, &rendered)?;
    Ok(entries.len())
}

fn validate_filter(filter: &ExportFilter) -> Result<(), BelayError> {
    if filter.tag.as_ref().is_some_and(|tag| tag.trim().is_empty()) {
        return validation("export tag must not be empty");
    }
    if let Some(display_id) = &filter.display_id {
        crate::entry::parse_display_id(display_id)?;
    }
    Ok(())
}

fn load_entries(
    repository: &Repository,
    filter: &ExportFilter,
) -> Result<Vec<ExportEntry>, BelayError> {
    let database_path = repository.database_path();
    let mut connection = crate::database::open_read_only(&database_path)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    let entry_type = filter.entry_type.map(|value| value.to_string());
    let status = filter.status.map(|value| value.to_string());
    let mut statement = transaction
        .prepare(
            "
            SELECT entry.id, entry.source_path
            FROM entries entry
            WHERE (?1 IS NULL OR entry.type = ?1)
              AND (?2 IS NULL OR entry.status = ?2)
              AND (?3 IS NULL OR EXISTS (
                  SELECT 1 FROM entry_tags tag
                  WHERE tag.entry_id = entry.id AND tag.tag = ?3
              ))
              AND (?4 IS NULL OR entry.display_id = ?4)
            ORDER BY entry.type, entry.display_id
            ",
        )
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    let rows = statement
        .query_map(
            params![
                entry_type.as_deref(),
                status.as_deref(),
                filter.tag.as_deref(),
                filter.display_id.as_deref()
            ],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .map_err(|source| BelayError::sqlite(&database_path, source))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    drop(statement);

    let entries = rows
        .into_iter()
        .map(|(internal_id, source_path)| {
            let entry = store::load_entry(&transaction, &database_path, internal_id)?;
            crate::contract::validate_entry_projection(repository, &entry)?;
            Ok(export_entry(entry, source_path.unwrap_or_default()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    transaction
        .commit()
        .map_err(|source| BelayError::sqlite(&database_path, source))?;
    Ok(entries)
}

fn export_entry(entry: Entry, source_path: String) -> ExportEntry {
    ExportEntry {
        schema_version: MARKDOWN_SCHEMA_VERSION,
        id: entry.display_id,
        entry_type: entry.entry_type,
        title: entry.title,
        status: entry.status,
        created_at: entry.created_at,
        updated_at: entry.updated_at,
        revision: entry.revision,
        tags: entry.tags,
        links: entry.links,
        metadata: entry.metadata,
        body: entry.body,
        source_path,
    }
}

fn render_markdown(entries: &[ExportEntry]) -> Vec<u8> {
    let mut output = String::from(
        "# belay-trace export\n\n\
         > External point-in-time snapshot. This file is not a managed mirror and is not an import or rebuild input.\n",
    );
    for entry in entries {
        output.push_str("\n---\n\n");
        output.push_str(&format!("# {}: {}\n\n", entry.id, entry.title));
        output.push_str(&format!(
            "- Type: {}\n- Status: {}\n- Created: {}\n- Updated: {}\n- Revision: {}\n- Source: {}\n",
            entry.entry_type,
            entry.status,
            entry.created_at,
            entry.updated_at,
            entry.revision,
            entry.source_path
        ));
        if !entry.tags.is_empty() {
            output.push_str(&format!("- Tags: {}\n", entry.tags.join(", ")));
        }
        if !entry.metadata.is_empty() {
            let metadata = serde_json::to_string(&entry.metadata).unwrap_or_else(|_| "{}".into());
            output.push_str(&format!("- Metadata: `{metadata}`\n"));
        }
        if !entry.links.is_empty() {
            output.push_str("\n## Links\n\n");
            for link in &entry.links {
                output.push_str(&format!("- {} {}\n", link.relation, link.id));
            }
        }
        output.push_str("\n## Body\n\n");
        output.push_str(&entry.body);
        output.push('\n');
    }
    output.into_bytes()
}

fn render_ndjson(entries: &[ExportEntry]) -> Result<Vec<u8>, BelayError> {
    let mut output = Vec::new();
    for entry in entries {
        serde_json::to_writer(&mut output, entry).map_err(serialization_error)?;
        output.push(b'\n');
    }
    Ok(output)
}

fn validate_destination(repository: &Repository, output: &Path) -> Result<PathBuf, BelayError> {
    let parent = output.parent().ok_or_else(|| BelayError::Validation {
        message: format!("export destination {} has no parent", output.display()),
    })?;
    let managed_root = fs::canonicalize(&repository.belay_dir).map_err(|source| {
        BelayError::io(
            "resolve managed belay directory",
            &repository.belay_dir,
            source,
        )
    })?;
    let projected_parent = resolve_existing_ancestor(parent)?;
    if projected_parent.starts_with(&managed_root) {
        return validation(format!(
            "export destination {} is inside managed belay state {}; choose an external path",
            output.display(),
            managed_root.display()
        ));
    }
    #[cfg(unix)]
    create_directory_no_follow(&projected_parent)?;
    #[cfg(not(unix))]
    fs::create_dir_all(parent)
        .map_err(|source| BelayError::io("create export directory", parent, source))?;
    let resolved_parent = fs::canonicalize(&projected_parent)
        .map_err(|source| BelayError::io("resolve export directory", parent, source))?;
    if output.file_name().is_none() {
        return validation(format!(
            "export destination {} must name a file",
            output.display()
        ));
    }
    Ok(resolved_parent)
}

fn resolve_existing_ancestor(path: &Path) -> Result<PathBuf, BelayError> {
    let mut ancestor = path;
    let mut remainder = Vec::new();
    while !ancestor.exists() {
        let name = ancestor.file_name().ok_or_else(|| BelayError::Validation {
            message: format!("export directory {} cannot be resolved", path.display()),
        })?;
        remainder.push(name.to_os_string());
        ancestor = ancestor.parent().ok_or_else(|| BelayError::Validation {
            message: format!(
                "export directory {} has no existing ancestor",
                path.display()
            ),
        })?;
    }
    let mut resolved = fs::canonicalize(ancestor)
        .map_err(|source| BelayError::io("resolve export directory", ancestor, source))?;
    for component in remainder.iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn write_atomic(repository: &Repository, path: &Path, contents: &[u8]) -> Result<(), BelayError> {
    let resolved_parent = validate_destination(repository, path)?;
    #[cfg(unix)]
    {
        write_atomic_unix(path, &resolved_parent, contents)
    }
    #[cfg(not(unix))]
    {
        write_atomic_portable(path, contents)
    }
}

#[cfg(unix)]
fn write_atomic_unix(
    path: &Path,
    resolved_parent: &Path,
    contents: &[u8],
) -> Result<(), BelayError> {
    let parent = open_directory_no_follow(resolved_parent)?;
    let name = path.file_name().ok_or_else(|| BelayError::Validation {
        message: format!("export destination {} must name a file", path.display()),
    })?;
    let mode = match rustix::fs::statat(&parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat)
            if rustix::fs::FileType::from_raw_mode(stat.st_mode)
                == rustix::fs::FileType::RegularFile =>
        {
            Mode::from_raw_mode(stat.st_mode)
        }
        Ok(_) => {
            return validation(format!(
                "export destination {} must be a regular file",
                path.display()
            ));
        }
        Err(rustix::io::Errno::NOENT) => Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::ROTH,
        Err(source) => return Err(unix_io("inspect export destination", path, source)),
    };
    let temporary = allocate_temporary_at(&parent, name, path)?;
    let result = (|| {
        let fd = rustix::fs::openat(
            &parent,
            &temporary,
            OFlags::WRONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|source| unix_io("open temporary export", path, source))?;
        rustix::fs::fchmod(&fd, mode)
            .map_err(|source| unix_io("set export permissions", path, source))?;
        let mut file = fs::File::from(fd);
        file.write_all(contents)
            .map_err(|source| BelayError::io("write temporary export", path, source))?;
        file.sync_all()
            .map_err(|source| BelayError::io("flush temporary export", path, source))?;
        rustix::fs::renameat(&parent, &temporary, &parent, name)
            .map_err(|source| unix_io("replace export", path, source))?;
        rustix::fs::fsync(&parent)
            .map_err(|source| unix_io("flush export directory", resolved_parent, source))
    })();
    if result.is_err() {
        let _ = rustix::fs::unlinkat(&parent, &temporary, AtFlags::empty());
    }
    result
}

#[cfg(unix)]
fn open_directory_no_follow(path: &Path) -> Result<OwnedFd, BelayError> {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut fd = rustix::fs::open("/", flags, Mode::empty())
        .map_err(|source| unix_io("open export directory root", path, source))?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(component) => {
                fd = rustix::fs::openat(&fd, component, flags, Mode::empty())
                    .map_err(|source| unix_io("open export directory", path, source))?;
            }
            _ => {
                return validation(format!(
                    "export directory {} must use a resolved absolute path",
                    path.display()
                ));
            }
        }
    }
    Ok(fd)
}

#[cfg(unix)]
fn create_directory_no_follow(path: &Path) -> Result<(), BelayError> {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut fd = rustix::fs::open("/", flags, Mode::empty())
        .map_err(|source| unix_io("open export directory root", path, source))?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(component) => {
                match rustix::fs::openat(&fd, component, flags, Mode::empty()) {
                    Ok(next) => fd = next,
                    Err(rustix::io::Errno::NOENT) => {
                        let created = match rustix::fs::mkdirat(
                            &fd,
                            component,
                            Mode::RUSR
                                | Mode::WUSR
                                | Mode::XUSR
                                | Mode::RGRP
                                | Mode::XGRP
                                | Mode::ROTH
                                | Mode::XOTH,
                        ) {
                            Ok(()) => true,
                            Err(rustix::io::Errno::EXIST) => false,
                            Err(source) => {
                                return Err(unix_io("create export directory", path, source));
                            }
                        };
                        if created {
                            rustix::fs::fsync(&fd).map_err(|source| {
                                unix_io("flush export directory parent", path, source)
                            })?;
                        }
                        fd = rustix::fs::openat(&fd, component, flags, Mode::empty())
                            .map_err(|source| unix_io("open export directory", path, source))?;
                    }
                    Err(source) => {
                        return Err(unix_io("open export directory", path, source));
                    }
                }
            }
            _ => {
                return validation(format!(
                    "export directory {} must use a resolved absolute path",
                    path.display()
                ));
            }
        }
    }
    rustix::fs::fsync(&fd).map_err(|source| unix_io("flush export directory", path, source))
}

#[cfg(unix)]
fn allocate_temporary_at(
    parent: &OwnedFd,
    name: &OsStr,
    path: &Path,
) -> Result<OsString, BelayError> {
    for attempt in 0..100_u32 {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temporary = OsString::from(format!(
            ".{}.tmp-{}-{nonce}-{attempt}",
            name.to_string_lossy(),
            std::process::id()
        ));
        match rustix::fs::openat(
            parent,
            &temporary,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        ) {
            Ok(fd) => {
                drop(fd);
                return Ok(temporary);
            }
            Err(rustix::io::Errno::EXIST) => continue,
            Err(source) => return Err(unix_io("create temporary export", path, source)),
        }
    }
    validation(format!(
        "could not allocate a temporary file beside {}",
        path.display()
    ))
}

#[cfg(unix)]
fn unix_io(action: &'static str, path: &Path, source: rustix::io::Errno) -> BelayError {
    BelayError::io(
        action,
        path,
        std::io::Error::from_raw_os_error(source.raw_os_error()),
    )
}

#[cfg(not(unix))]
fn write_atomic_portable(path: &Path, contents: &[u8]) -> Result<(), BelayError> {
    let parent = path.parent().ok_or_else(|| BelayError::Validation {
        message: format!("export destination {} has no parent", path.display()),
    })?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| BelayError::Validation {
            message: format!("export destination {} is not valid UTF-8", path.display()),
        })?;
    let temporary = parent.join(format!(".{name}.tmp-{}-{nonce}", std::process::id()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|source| BelayError::io("create temporary export", &temporary, source))?;
        file.write_all(contents)
            .map_err(|source| BelayError::io("write temporary export", &temporary, source))?;
        file.sync_all()
            .map_err(|source| BelayError::io("flush temporary export", &temporary, source))?;
        fs::rename(&temporary, path)
            .map_err(|source| BelayError::io("replace export", path, source))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn absolute_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn serialization_error(source: serde_json::Error) -> BelayError {
    BelayError::Validation {
        message: format!("could not serialize export: {source}"),
    }
}

fn validation<T>(message: impl Into<String>) -> Result<T, BelayError> {
    Err(BelayError::Validation {
        message: message.into(),
    })
}
