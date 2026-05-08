use std::fs::{self, File};
use std::io::{Cursor, Seek};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use zip::ZipArchive;

use crate::model::ArchiveEntry;

/// Read a zip archive from memory and return metadata for each entry.
pub fn read_zip_entries(data: &[u8]) -> Result<Vec<ArchiveEntry>> {
    let mut archive =
        ZipArchive::new(Cursor::new(data)).context("failed to open zip archive from bytes")?;
    let mut entries = Vec::new();

    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .context("failed to access zip entry")?;
        entries.push(ArchiveEntry {
            path: file.name().to_string(),
            size: file.size(),
        });
    }

    Ok(entries)
}

/// Extract a zip archive from memory into a deterministic output directory.
pub fn extract_zip_bytes(data: &[u8], out_dir: &Path) -> Result<Vec<PathBuf>> {
    let reader = Cursor::new(data);
    extract_zip_reader(reader, out_dir)
}

/// Extract a zip archive from a file on disk.
pub fn extract_zip_file(zip_path: &Path, out_dir: &Path) -> Result<Vec<PathBuf>> {
    let file = File::open(zip_path)
        .with_context(|| format!("failed to open zip file: {}", zip_path.display()))?;
    extract_zip_reader(file, out_dir)
}

fn extract_zip_reader<R: std::io::Read + Seek>(reader: R, out_dir: &Path) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create output directory: {}", out_dir.display()))?;
    let mut archive = ZipArchive::new(reader).context("failed to read zip archive")?;
    let mut written_paths = Vec::new();

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .context("failed to access zip entry")?;
        let safe_relative = sanitize_zip_path(file.name())?;
        let out_path = out_dir.join(&safe_relative);

        if file.is_dir() {
            fs::create_dir_all(&out_path).with_context(|| {
                format!(
                    "failed to create extracted directory: {}",
                    out_path.display()
                )
            })?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create parent directory: {}", parent.display())
            })?;
        }

        let mut output = File::create(&out_path)
            .with_context(|| format!("failed to create extracted file: {}", out_path.display()))?;
        std::io::copy(&mut file, &mut output)
            .with_context(|| format!("failed to extract file: {}", out_path.display()))?;
        written_paths.push(out_path);
    }

    written_paths.sort();
    Ok(written_paths)
}

fn sanitize_zip_path(path: &str) -> Result<PathBuf> {
    let candidate = Path::new(path);
    let mut clean = PathBuf::new();

    for component in candidate.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::RootDir | Component::ParentDir | Component::Prefix(_) => {
                bail!("zip entry contains unsafe path component: {path}")
            }
        }
    }

    Ok(clean)
}

/// Recursively scan files under a directory, optionally filtering by suffix.
pub fn scan_files(root: &Path, suffixes: Option<&[&str]>) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry
            .with_context(|| format!("failed while scanning directory tree: {}", root.display()))?;
        if entry.file_type().is_file() {
            let path = entry.into_path();
            let include = match suffixes {
                None => true,
                Some(wanted) => wanted.iter().any(|suffix| {
                    path.to_string_lossy()
                        .to_ascii_lowercase()
                        .ends_with(&suffix.to_ascii_lowercase())
                }),
            };
            if include {
                paths.push(path);
            }
        }
    }
    paths.sort();
    Ok(paths)
}
