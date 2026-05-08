use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::to_string_pretty;

use crate::api;
use crate::db;
use crate::decrypt;
use crate::error::BuddyCastError;
use crate::model::{NormalizedItem, WorkflowResult};
use crate::progress::{self, ProgressHandle};
use crate::{archive, subtitle};

/// Output toggles and retention settings shared by all workflow entry points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkflowOptions {
    pub export_ass: bool,
    pub export_srt: bool,
    pub parse_db_files: bool,
    /// When true, keep the copied encrypted zip package in the output directory.
    pub keep_encrypted_zip: bool,
}

/// Network-related parameters for the online workflow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FetchRequest<'a> {
    pub user: &'a str,
    pub password: &'a str,
    pub asset_id: &'a str,
    pub timeout: f64,
    pub retries: usize,
    pub retry_delay_seconds: f64,
}

/// Offline input paths resolved by the caller or CLI layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineRequest {
    pub asset_id: String,
    pub list_json_path: PathBuf,
    pub encrypted_zip_path: PathBuf,
}

/// Small resolved package object used to keep source resolution separate from shared processing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedPackage {
    item: NormalizedItem,
    encrypted_zip_bytes: Vec<u8>,
    contents_json_text: Option<String>,
}

/// Decrypt, extract, and post-process a local encrypted zip file.
pub fn process_local_encrypted_zip(
    encrypted_zip_path: &Path,
    out_dir: &Path,
    options: WorkflowOptions,
) -> Result<WorkflowResult> {
    process_local_encrypted_zip_impl(encrypted_zip_path, out_dir, options, None)
}

/// Extract and post-process a local decrypted zip file.
pub fn process_local_decrypted_zip(
    decrypted_zip_path: &Path,
    out_dir: &Path,
    options: WorkflowOptions,
) -> Result<WorkflowResult> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create output directory: {}", out_dir.display()))?;
    let copied_zip_path = out_dir.join(
        decrypted_zip_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "archive.zip".to_string()),
    );
    if copied_zip_path != decrypted_zip_path {
        let bytes = fs::read(decrypted_zip_path).with_context(|| {
            format!(
                "failed to read decrypted zip from {}",
                decrypted_zip_path.display()
            )
        })?;
        write_overwriting_path(&copied_zip_path, &bytes, "decrypted zip copy")?;
    }

    let extract_dir = out_dir.join(
        copied_zip_path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "archive".to_string()),
    );
    reset_directory(&extract_dir)?;
    archive::extract_zip_file(&copied_zip_path, &extract_dir)?;
    decrypt_embedded_assets(&extract_dir)?;
    let subtitle_outputs = write_subtitles(&extract_dir, options.export_ass, options.export_srt)?;
    let db_reports = if options.parse_db_files {
        write_db_reports(&extract_dir)?
    } else {
        Vec::new()
    };

    Ok(WorkflowResult {
        item: None,
        encrypted_zip_path: None,
        decrypted_zip_path: Some(copied_zip_path),
        extract_dir: Some(extract_dir),
        subtitle_outputs,
        db_reports,
    })
}

/// Run the workflow when the caller already has a resolved normalized item and encrypted bytes.
pub fn process_resolved_item(
    item: NormalizedItem,
    encrypted_zip_bytes: &[u8],
    out_dir: &Path,
    options: WorkflowOptions,
) -> Result<WorkflowResult> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create output directory: {}", out_dir.display()))?;
    let encrypted_zip_path = out_dir.join(format!("{}.encrypted.zip", item.asset_id));
    write_overwriting_path(&encrypted_zip_path, encrypted_zip_bytes, "encrypted zip")?;

    let work_name = sanitize_work_name(&item.title, &item.asset_id);
    let mut result = process_local_encrypted_zip_impl(
        &encrypted_zip_path,
        out_dir,
        options,
        Some(work_name.as_str()),
    )?;

    if !options.keep_encrypted_zip {
        if let Some(encrypted_zip_path) = &result.encrypted_zip_path
            && encrypted_zip_path.exists()
        {
            fs::remove_file(encrypted_zip_path).with_context(|| {
                format!(
                    "failed to remove encrypted zip: {}",
                    encrypted_zip_path.display()
                )
            })?;
        }
        result.encrypted_zip_path = None;
    }

    result.item = Some(item);
    Ok(result)
}

/// Fetch all required data from the remote API and run the full workflow.
pub fn fetch_from_api_by_asset_id(
    request: FetchRequest<'_>,
    out_dir: &Path,
    options: WorkflowOptions,
) -> Result<WorkflowResult> {
    let resolved = resolve_online_package(request)?;
    process_resolved_package(resolved, out_dir, options)
}

/// Resolve local offline inputs and run the workflow without network access.
pub fn fetch_offline_by_asset_id(
    request: &OfflineRequest,
    out_dir: &Path,
    options: WorkflowOptions,
) -> Result<WorkflowResult> {
    let resolved = resolve_offline_package(request)?;
    process_resolved_package(resolved, out_dir, options)
}

fn process_local_encrypted_zip_impl(
    encrypted_zip_path: &Path,
    out_dir: &Path,
    options: WorkflowOptions,
    extract_dir_name_override: Option<&str>,
) -> Result<WorkflowResult> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create output directory: {}", out_dir.display()))?;
    let encrypted_bytes = fs::read(encrypted_zip_path).with_context(|| {
        format!(
            "failed to read encrypted zip: {}",
            encrypted_zip_path.display()
        )
    })?;
    let decrypt_progress =
        progress::bytes_bar(encrypted_bytes.len() as u64, "Decrypting package...");
    let decrypted_bytes = decrypt_with_progress(&encrypted_bytes, &decrypt_progress);
    decrypt_progress.finish_and_clear();
    if !decrypt::is_zip_bytes(&decrypted_bytes) {
        return Err(BuddyCastError::InvalidDecryptedZip.into());
    }

    let stem = encrypted_name_stem(encrypted_zip_path);
    let decrypted_zip_path = out_dir.join(format!("{stem}.zip"));
    write_overwriting_path(&decrypted_zip_path, &decrypted_bytes, "decrypted zip")?;

    let extract_dir_name = extract_dir_name_override.unwrap_or(&stem);
    let extract_dir = out_dir.join(extract_dir_name);
    reset_directory(&extract_dir)?;
    archive::extract_zip_bytes(&decrypted_bytes, &extract_dir)?;
    decrypt_embedded_assets(&extract_dir)?;

    let subtitle_outputs = write_subtitles(&extract_dir, options.export_ass, options.export_srt)?;
    let db_reports = if options.parse_db_files {
        write_db_reports(&extract_dir)?
    } else {
        Vec::new()
    };

    let mut encrypted_copy_path = out_dir.join(
        encrypted_zip_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{stem}.encrypted.zip")),
    );
    if encrypted_copy_path == decrypted_zip_path {
        encrypted_copy_path = out_dir.join(format!("{stem}.encrypted.zip"));
    }
    if encrypted_copy_path != encrypted_zip_path {
        write_overwriting_path(&encrypted_copy_path, &encrypted_bytes, "encrypted zip copy")?;
    }

    Ok(WorkflowResult {
        item: None,
        encrypted_zip_path: Some(if encrypted_copy_path != encrypted_zip_path {
            encrypted_copy_path
        } else {
            encrypted_zip_path.to_path_buf()
        }),
        decrypted_zip_path: Some(decrypted_zip_path),
        extract_dir: Some(extract_dir),
        subtitle_outputs,
        db_reports,
    })
}

fn resolve_online_package(request: FetchRequest<'_>) -> Result<ResolvedPackage> {
    let list_json = api::fetch_content_list(
        request.user,
        request.password,
        request.timeout,
        request.retries,
        request.retry_delay_seconds,
    )?;
    let raw_item = api::find_item_by_asset_id(&list_json, request.asset_id)?;
    let item = api::normalize_item(&raw_item)?;
    let encrypted_zip_bytes = api::download_encrypted_zip(
        &raw_item,
        request.timeout,
        request.retries,
        request.retry_delay_seconds,
    )?;

    Ok(ResolvedPackage {
        item,
        encrypted_zip_bytes,
        contents_json_text: Some(
            to_string_pretty(&list_json).context("failed to serialize fetched content list")?,
        ),
    })
}

fn resolve_offline_package(request: &OfflineRequest) -> Result<ResolvedPackage> {
    let list_json = api::load_content_list(&request.list_json_path)?;
    let raw_item = api::find_item_by_asset_id(&list_json, &request.asset_id)?;
    let item = api::normalize_item(&raw_item)?;
    let encrypted_zip_bytes = fs::read(&request.encrypted_zip_path).with_context(|| {
        format!(
            "failed to read encrypted zip: {}",
            request.encrypted_zip_path.display()
        )
    })?;
    let contents_json_text = fs::read_to_string(&request.list_json_path).with_context(|| {
        format!(
            "failed to read content list: {}",
            request.list_json_path.display()
        )
    })?;

    Ok(ResolvedPackage {
        item,
        encrypted_zip_bytes,
        contents_json_text: Some(contents_json_text),
    })
}

fn process_resolved_package(
    resolved: ResolvedPackage,
    out_dir: &Path,
    options: WorkflowOptions,
) -> Result<WorkflowResult> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create output directory: {}", out_dir.display()))?;
    if let Some(contents_json_text) = &resolved.contents_json_text {
        let contents_json_path = out_dir.join("contents.json");
        fs::write(&contents_json_path, contents_json_text).with_context(|| {
            format!(
                "failed to write content list: {}",
                contents_json_path.display()
            )
        })?;
    }

    process_resolved_item(
        resolved.item,
        &resolved.encrypted_zip_bytes,
        out_dir,
        options,
    )
}

fn write_subtitles(extract_dir: &Path, export_ass: bool, export_srt: bool) -> Result<Vec<PathBuf>> {
    let mut outputs = Vec::new();
    for subtitle_path in archive::scan_files(extract_dir, Some(&[".decrypted", ".oxk"]))? {
        let file_name = subtitle_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let is_supported = file_name.ends_with(".oxk.decrypted")
            || subtitle_path.extension().and_then(|ext| ext.to_str()) == Some("oxk");
        if !is_supported {
            continue;
        }

        let events = match subtitle::parse_oxk_file(&subtitle_path) {
            Ok(events) => events,
            Err(_) => continue,
        };

        let base_path =
            if subtitle_path.extension().and_then(|ext| ext.to_str()) == Some("decrypted") {
                subtitle_path.with_extension("")
            } else {
                subtitle_path.clone()
            };

        if export_ass {
            let ass_path = base_path.with_extension("ass");
            fs::write(&ass_path, subtitle::render_ass(&events))
                .with_context(|| format!("failed to write ASS subtitle: {}", ass_path.display()))?;
            outputs.push(ass_path);
        }
        if export_srt {
            let srt_path = base_path.with_extension("srt");
            fs::write(&srt_path, subtitle::render_srt(&events))
                .with_context(|| format!("failed to write SRT subtitle: {}", srt_path.display()))?;
            outputs.push(srt_path);
        }
    }
    Ok(outputs)
}

fn write_db_reports(extract_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut outputs = Vec::new();
    for db_path in archive::scan_files(extract_dir, Some(&[".db"]))? {
        let display_path = db_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| db_path.display().to_string());
        let report = db::parse_db_file(&db_path, Some(&display_path))?;
        let report_path = db_path.with_extension(format!(
            "{}.json",
            db_path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("db")
        ));
        fs::write(
            &report_path,
            to_string_pretty(&report).context("failed to serialize db report")?,
        )
        .with_context(|| format!("failed to write db report: {}", report_path.display()))?;
        outputs.push(report_path);
    }
    Ok(outputs)
}

fn decrypt_embedded_assets(root: &Path) -> Result<()> {
    let candidate_paths = archive::scan_files(root, None)?
        .into_iter()
        .filter(|path| should_try_embedded_decrypt(path, root))
        .collect::<Vec<_>>();
    if candidate_paths.is_empty() {
        return Ok(());
    }

    let progress_handle = progress::count_bar(
        candidate_paths.len() as u64,
        "Decrypting embedded assets...",
    );
    let mut invalid_candidates = Vec::new();

    for path in candidate_paths {
        let encrypted_bytes = fs::read(&path)
            .with_context(|| format!("failed to read extracted file: {}", path.display()))?;
        let validation = classify_embedded_asset(&path, root, &encrypted_bytes);
        match validation {
            EmbeddedAssetState::AlreadyPlaintext => {
                progress_handle.inc(1);
                continue;
            }
            EmbeddedAssetState::SkipUnknown => {
                progress_handle.inc(1);
                continue;
            }
            EmbeddedAssetState::NeedsDecrypt => {}
        }

        let decrypted_bytes = decrypt::decrypt_bytes(&encrypted_bytes);
        if !is_valid_decrypted_candidate(&path, root, &decrypted_bytes) {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(path.as_path())
                .display()
                .to_string();
            invalid_candidates.push(relative);
            progress_handle.inc(1);
            continue;
        }

        fs::write(&path, &decrypted_bytes)
            .with_context(|| format!("failed to rewrite decrypted file: {}", path.display()))?;
        progress_handle.inc(1);
    }

    progress_handle.finish_and_clear();
    if invalid_candidates.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "embedded asset decryption failed validation for {} file(s): {}",
            invalid_candidates.len(),
            invalid_candidates.join(", ")
        ))
    }
}

fn should_try_embedded_decrypt(path: &Path, root: &Path) -> bool {
    embedded_asset_kind(path, root).is_some()
}

fn classify_embedded_asset(path: &Path, root: &Path, bytes: &[u8]) -> EmbeddedAssetState {
    match embedded_asset_kind(path, root) {
        Some(EmbeddedAssetKind::Oxk) => {
            if looks_like_xml(bytes) {
                EmbeddedAssetState::AlreadyPlaintext
            } else {
                EmbeddedAssetState::NeedsDecrypt
            }
        }
        Some(EmbeddedAssetKind::Csv) => {
            if looks_like_text(bytes) {
                EmbeddedAssetState::AlreadyPlaintext
            } else {
                EmbeddedAssetState::NeedsDecrypt
            }
        }
        Some(EmbeddedAssetKind::Db) => {
            if db::parse_db_bytes(bytes, &path.display().to_string()).is_ok() {
                EmbeddedAssetState::AlreadyPlaintext
            } else {
                EmbeddedAssetState::NeedsDecrypt
            }
        }
        Some(EmbeddedAssetKind::Image) => {
            if looks_like_supported_image_header(bytes) {
                EmbeddedAssetState::AlreadyPlaintext
            } else {
                EmbeddedAssetState::NeedsDecrypt
            }
        }
        None => EmbeddedAssetState::SkipUnknown,
    }
}

fn is_valid_decrypted_candidate(path: &Path, root: &Path, bytes: &[u8]) -> bool {
    match embedded_asset_kind(path, root) {
        Some(EmbeddedAssetKind::Oxk) => looks_like_xml(bytes),
        Some(EmbeddedAssetKind::Csv) => looks_like_text(bytes),
        Some(EmbeddedAssetKind::Db) => {
            db::parse_db_bytes(bytes, &path.display().to_string()).is_ok()
        }
        Some(EmbeddedAssetKind::Image) => looks_like_supported_image_header(bytes),
        None => false,
    }
}

fn looks_like_xml(bytes: &[u8]) -> bool {
    let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(128)]);
    let trimmed = prefix.trim_start_matches('\u{feff}').trim_start();
    trimmed.starts_with("<?xml") || trimmed.starts_with('<')
}

fn looks_like_text(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok()
}

fn looks_like_supported_image_header(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || bytes.starts_with(&[0xFF, 0xD8, 0xFF])
        || bytes.starts_with(b"BM")
        || bytes.starts_with(b"II*\0")
        || bytes.starts_with(b"MM\0*")
        || bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP")
}

fn embedded_asset_kind(path: &Path, root: &Path) -> Option<EmbeddedAssetKind> {
    let extension = path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "oxk" => return Some(EmbeddedAssetKind::Oxk),
        "csv" => return Some(EmbeddedAssetKind::Csv),
        "db" => return Some(EmbeddedAssetKind::Db),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tif" | "tiff" | "webp" => {
            return Some(EmbeddedAssetKind::Image);
        }
        _ => {}
    }

    if let Ok(relative_path) = path.strip_prefix(root) {
        for component in relative_path.components() {
            let component = component.as_os_str().to_string_lossy().to_ascii_lowercase();
            match component.as_str() {
                "img" | "pict" => return Some(EmbeddedAssetKind::Image),
                "oxk" => return Some(EmbeddedAssetKind::Oxk),
                "csv" => return Some(EmbeddedAssetKind::Csv),
                "db" => return Some(EmbeddedAssetKind::Db),
                _ => {}
            }
        }
    }

    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmbeddedAssetKind {
    Oxk,
    Csv,
    Db,
    Image,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmbeddedAssetState {
    AlreadyPlaintext,
    NeedsDecrypt,
    SkipUnknown,
}

fn decrypt_with_progress(encrypted_bytes: &[u8], progress_handle: &ProgressHandle) -> Vec<u8> {
    let mut decrypted = Vec::with_capacity(encrypted_bytes.len());
    for chunk in encrypted_bytes.chunks(8 * 1024) {
        decrypted.extend(
            chunk
                .iter()
                .map(|byte| decrypt::DECRYPTION_MAP[*byte as usize]),
        );
        progress_handle.inc(chunk.len() as u64);
    }
    decrypted
}

fn encrypted_name_stem(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "archive.encrypted.zip".to_string());
    if let Some(stem) = name.strip_suffix(".encrypted.zip") {
        stem.to_string()
    } else if let Some(stem) = path.file_stem() {
        stem.to_string_lossy().to_string()
    } else {
        "archive".to_string()
    }
}

fn write_overwriting_path(path: &Path, bytes: &[u8], label: &str) -> Result<()> {
    remove_path_if_exists(path)?;
    fs::write(path, bytes).with_context(|| format!("failed to write {label}: {}", path.display()))
}

fn reset_directory(path: &Path) -> Result<()> {
    if path.exists() {
        remove_path_if_exists(path)?;
    }
    fs::create_dir_all(path)
        .with_context(|| format!("failed to create directory: {}", path.display()))
}

fn remove_path_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to read metadata for path: {}", path.display()))?;
    if metadata.is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove directory: {}", path.display()))
    } else {
        fs::remove_file(path).with_context(|| format!("failed to remove file: {}", path.display()))
    }
}

fn sanitize_work_name(title: &str, asset_id: &str) -> String {
    let trimmed = title
        .replace("<br/>", " ")
        .trim()
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            '\r' | '\n' | '\t' => ' ',
            _ => character,
        })
        .collect::<String>();
    let collapsed = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        asset_id.to_string()
    } else {
        collapsed
    }
}
