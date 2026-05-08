use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Normalized representation of a content-list item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedItem {
    pub item_id: String,
    pub asset_id: String,
    pub url: String,
    pub size: String,
    pub title: String,
    pub has_sub_tag: bool,
    pub size_bytes: u64,
}

/// Simple zip entry metadata used by archive helpers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub path: String,
    pub size: u64,
}

/// Parsed subtitle event from the OXK XML format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleEvent {
    pub start: f64,
    pub end: f64,
    pub alignment: String,
    pub arrangement: String,
    pub text: String,
    pub comment: String,
}

/// File outputs produced by a workflow execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub item: Option<NormalizedItem>,
    pub encrypted_zip_path: Option<PathBuf>,
    pub decrypted_zip_path: Option<PathBuf>,
    pub extract_dir: Option<PathBuf>,
    pub subtitle_outputs: Vec<PathBuf>,
    pub db_reports: Vec<PathBuf>,
}
