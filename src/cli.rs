use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use serde_json::json;

use crate::api::{self, DEFAULT_PASSWORD, DEFAULT_USER};
use crate::error::BuddyCastError;
use crate::model::{NormalizedItem, WorkflowResult};
use crate::workflow::{self, FetchRequest, OfflineRequest, WorkflowOptions};

const DEFAULT_RETRIES: usize = 3;
const DEFAULT_DOWNLOAD_DIR: &str = "downloads";

/// Run the command line user interface.
pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Getinfo {
            timeout,
            save,
            limit,
        } => run_getinfo(timeout, save, limit),
        Command::Fetch {
            asset_id,
            timeout,
            out,
            keep_encrypted_zip,
            srt,
            offline,
        } => run_fetch(&asset_id, timeout, &out, keep_encrypted_zip, srt, offline),
    }
}

#[derive(Debug, Parser)]
#[command(name = "buddy_cast")]
#[command(about = "Fetch, decrypt, and inspect UDCast packages")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Fetch the content list and print normalized entries.
    Getinfo {
        #[arg(long, default_value_t = 60.0)]
        timeout: f64,
        #[arg(long)]
        save: Option<PathBuf>,
        #[arg(long, default_value = "50")]
        limit: DisplayLimit,
    },
    /// Download or resolve a package by asset id and run the extraction workflow.
    Fetch {
        asset_id: String,
        #[arg(long, default_value_t = 60.0)]
        timeout: f64,
        #[arg(long, default_value = "downloads")]
        out: PathBuf,
        #[arg(long)]
        keep_encrypted_zip: bool,
        #[arg(long)]
        srt: bool,
        #[arg(long)]
        offline: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DisplayLimit {
    All,
    Count(usize),
}

impl std::str::FromStr for DisplayLimit {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        if value.eq_ignore_ascii_case("all") {
            return Ok(Self::All);
        }
        let parsed = value
            .parse::<usize>()
            .map_err(|_| format!("invalid limit `{value}`: use a positive integer or `all`"))?;
        if parsed == 0 {
            return Err("limit must be greater than zero or `all`".to_string());
        }
        Ok(Self::Count(parsed))
    }
}

fn run_getinfo(timeout: f64, save: Option<PathBuf>, limit: DisplayLimit) -> Result<()> {
    let list_json = api::fetch_content_list(
        DEFAULT_USER,
        DEFAULT_PASSWORD,
        timeout,
        DEFAULT_RETRIES,
        1.0,
    )?;
    if let Some(path) = save {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create save directory: {}", parent.display())
            })?;
        }
        fs::write(
            &path,
            serde_json::to_string_pretty(&list_json).context("failed to serialize content list")?,
        )
        .with_context(|| format!("failed to save content list: {}", path.display()))?;
    }

    let normalized = api::normalized_display_items(&list_json)?;
    let limited = apply_display_limit(&normalized, &limit);
    print_getinfo_table(limited);
    Ok(())
}

fn run_fetch(
    asset_id: &str,
    timeout: f64,
    out: &Path,
    keep_encrypted_zip: bool,
    srt: bool,
    offline: bool,
) -> Result<()> {
    let result = if offline {
        let (list_json_path, encrypted_zip_path) = resolve_offline_inputs(asset_id)?;
        workflow::fetch_offline_by_asset_id(
            &OfflineRequest {
                asset_id: asset_id.to_string(),
                list_json_path,
                encrypted_zip_path,
            },
            out,
            WorkflowOptions {
                export_ass: true,
                export_srt: srt,
                parse_db_files: true,
                keep_encrypted_zip,
            },
        )
        .map_err(|error| anyhow!("offline fetch failed: {error}"))?
    } else {
        workflow::fetch_from_api_by_asset_id(
            FetchRequest {
                user: DEFAULT_USER,
                password: DEFAULT_PASSWORD,
                asset_id,
                timeout,
                retries: DEFAULT_RETRIES,
                retry_delay_seconds: 1.0,
            },
            out,
            WorkflowOptions {
                export_ass: true,
                export_srt: srt,
                parse_db_files: true,
                keep_encrypted_zip,
            },
        )
        .map_err(|error| match error.downcast_ref::<BuddyCastError>() {
            Some(BuddyCastError::ItemNotFound(_)) => {
                anyhow!("online fetch failed: asset id not found in filtered remote contents list: {asset_id}")
            }
            _ => anyhow!("online fetch failed: {error}"),
        })?
    };

    print_result(&result)?;
    Ok(())
}

fn print_getinfo_table(items: &[NormalizedItem]) {
    let rows: Vec<[String; 3]> = items
        .iter()
        .map(|item| {
            [
                item.asset_id.clone(),
                if item.size.is_empty() {
                    "-".to_string()
                } else {
                    item.size.clone()
                },
                if item.title.is_empty() {
                    "-".to_string()
                } else {
                    sanitize_title_for_display(&item.title)
                },
            ]
        })
        .collect();

    let headers = ["ASSET_ID", "SIZE", "TITLE"];
    let max_widths = [24usize, 10usize, 60usize];
    let mut widths = [0usize; 3];

    for index in 0..headers.len() {
        let longest_row = rows.iter().map(|row| row[index].len()).max().unwrap_or(0);
        widths[index] = headers[index].len().max(longest_row).min(max_widths[index]);
    }

    let header_line = headers
        .iter()
        .enumerate()
        .map(|(index, header)| format!("{header:<width$}", width = widths[index]))
        .collect::<Vec<_>>()
        .join("  ");
    let separator = widths
        .iter()
        .map(|width| "-".repeat(*width))
        .collect::<Vec<_>>()
        .join("  ");

    println!("{header_line}");
    println!("{separator}");
    for row in rows {
        println!(
            "{}",
            row.iter()
                .enumerate()
                .map(|(index, value)| format!(
                    "{:<width$}",
                    truncate(value, widths[index]),
                    width = widths[index]
                ))
                .collect::<Vec<_>>()
                .join("  ")
        );
    }
}

fn apply_display_limit<'a>(
    items: &'a [NormalizedItem],
    limit: &DisplayLimit,
) -> &'a [NormalizedItem] {
    match limit {
        DisplayLimit::All => items,
        DisplayLimit::Count(count) => &items[..items.len().min(*count)],
    }
}

fn sanitize_title_for_display(title: &str) -> String {
    title.replace("<br/>", "")
}

fn truncate(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let char_count = text.chars().count();
    if char_count <= width {
        return text.to_string();
    }
    if width <= 3 {
        return text.chars().take(width).collect();
    }
    let mut result: String = text.chars().take(width - 3).collect();
    result.push_str("...");
    result
}

fn print_result(result: &WorkflowResult) -> Result<()> {
    let printable_item = result.item.as_ref().map(|item| {
        json!({
            "asset_id": item.asset_id,
            "size": item.size,
            "title": item.title,
        })
    });
    let printable = json!({
        "item": printable_item,
        "encrypted_zip_path": result.encrypted_zip_path.as_ref().map(|path| path.display().to_string()),
        "decrypted_zip_path": result.decrypted_zip_path.as_ref().map(|path| path.display().to_string()),
        "extract_dir": result.extract_dir.as_ref().map(|path| path.display().to_string()),
        "subtitle_outputs": result
            .subtitle_outputs
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>(),
        "db_reports": result
            .db_reports
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>(),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&printable).context("failed to serialize workflow result")?
    );
    Ok(())
}

fn resolve_offline_inputs(asset_id: &str) -> Result<(PathBuf, PathBuf)> {
    let list_json_path = PathBuf::from(DEFAULT_DOWNLOAD_DIR).join("contents.json");
    if !list_json_path.exists() {
        return Err(BuddyCastError::OfflineListMissing(list_json_path).into());
    }

    let list_json = api::load_content_list(&list_json_path)?;
    let raw_item = api::find_item_by_asset_id(&list_json, asset_id)?;
    let item = api::normalize_item(&raw_item)?;
    let encrypted_zip_path =
        PathBuf::from(DEFAULT_DOWNLOAD_DIR).join(format!("{}.encrypted.zip", item.asset_id));
    if !encrypted_zip_path.exists() {
        return Err(BuddyCastError::OfflinePackageMissing(encrypted_zip_path).into());
    }

    Ok((list_json_path, encrypted_zip_path))
}

#[cfg(test)]
mod tests {
    use super::{DisplayLimit, apply_display_limit, sanitize_title_for_display};
    use crate::model::NormalizedItem;

    #[test]
    fn sanitize_title_removes_html_break_tags() {
        assert_eq!(
            sanitize_title_for_display("第一行<br/>第二行"),
            "第一行第二行"
        );
    }

    #[test]
    fn display_limit_defaults_to_first_fifty_items() {
        let items = (0..60)
            .map(|index| NormalizedItem {
                item_id: index.to_string(),
                asset_id: format!("asset-{index}"),
                url: String::new(),
                size: "1".to_string(),
                title: format!("title-{index}"),
                has_sub_tag: true,
                size_bytes: 1,
            })
            .collect::<Vec<_>>();
        let limited = apply_display_limit(&items, &DisplayLimit::Count(50));
        assert_eq!(limited.len(), 50);
        assert_eq!(limited[0].asset_id, "asset-0");
        assert_eq!(limited[49].asset_id, "asset-49");
    }
}
