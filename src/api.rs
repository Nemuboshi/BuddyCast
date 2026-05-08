use std::fs;
use std::io::Read;
use std::time::Duration;

use anyhow::{Context, Result};
use base64::Engine as _;
use reqwest::blocking::Client;
use serde_json::Value;

use crate::error::BuddyCastError;
use crate::model::NormalizedItem;
use crate::progress::{self, ProgressHandle};

pub const BASE_URL: &str = "https://cms.palabra.jp";
pub const LIST_PATH: &str = "/masc/encrypt/list";
pub const DEFAULT_USER: &str = "UDeC4PTB";
pub const DEFAULT_PASSWORD: &str = "iqKJwKZJ";
pub const MINIMUM_VISIBLE_SIZE_BYTES: u64 = 100_000;

/// Build the HTTP Basic authorization header value used by the service.
pub fn auth_header(user: &str, password: &str) -> String {
    let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"));
    format!("Basic {token}")
}

/// Fetch the remote content list with a blocking HTTP client and simple retry logic.
pub fn fetch_content_list(
    user: &str,
    password: &str,
    timeout: f64,
    retries: usize,
    retry_delay_seconds: f64,
) -> Result<Value> {
    let progress_handle = progress::spinner("Fetching contents JSON...");
    let result = retry_bytes(retries, retry_delay_seconds, || {
        let client = Client::builder()
            .timeout(Duration::from_secs_f64(timeout))
            .build()
            .context("failed to build HTTP client")?;
        let response = client
            .get(format!("{BASE_URL}{LIST_PATH}"))
            .header("Authorization", auth_header(user, password))
            .header("User-Agent", "UDCast/5.3.2 Rust port")
            .send()
            .context("failed to fetch remote contents list")?
            .error_for_status()
            .context("remote contents list returned an error status")?;
        read_response_bytes(response, progress::hidden())
    })
    .and_then(|raw| {
        serde_json::from_slice(&raw).context("failed to decode remote contents list json")
    });

    match &result {
        Ok(_) => progress_handle.finish_and_clear(),
        Err(_) => progress_handle.abandon_with_message("Failed to fetch contents JSON."),
    }
    result
}

/// Load a content list from disk.
pub fn load_content_list(path: &std::path::Path) -> Result<Value> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read content list: {}", path.display()))?;
    serde_json::from_str(&text).context("failed to decode local content list json")
}

/// Return the raw delivery content items from the list JSON.
pub fn iter_delivery_contents(list_json: &Value) -> Result<Vec<Value>> {
    let items = list_json
        .get("contents")
        .and_then(|value| value.get("deliveryContents"))
        .ok_or(BuddyCastError::MissingDeliveryContents)?;
    let array = items
        .as_array()
        .ok_or(BuddyCastError::DeliveryContentsNotList)?;
    Ok(array.clone())
}

/// Normalize a raw content item into the compact model used by the CLI and workflow.
pub fn normalize_item(item: &Value) -> Result<NormalizedItem> {
    let contents = item
        .get("contents")
        .and_then(Value::as_object)
        .ok_or(BuddyCastError::MissingItemContents)?;

    let item_id = item
        .get("id")
        .and_then(Value::as_str)
        .ok_or(BuddyCastError::MissingItemId)?;
    let asset_id = contents
        .get("eTags")
        .or_else(|| contents.get("etag"))
        .and_then(Value::as_str)
        .ok_or(BuddyCastError::MissingAssetId)?;
    let url_path = contents
        .get("url")
        .and_then(Value::as_str)
        .ok_or(BuddyCastError::MissingItemUrl)?;
    let size = contents.get("size").and_then(Value::as_str).unwrap_or("");
    let title = item.get("title").and_then(Value::as_str).unwrap_or("");
    let tags = item
        .get("tags")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let url = if url_path.starts_with("http://") || url_path.starts_with("https://") {
        url_path.to_string()
    } else {
        format!("{BASE_URL}{url_path}")
    };

    Ok(NormalizedItem {
        item_id: item_id.to_string(),
        asset_id: asset_id.to_string(),
        url,
        size: size.to_string(),
        title: title.to_string(),
        has_sub_tag: tags.iter().any(has_sub_tag),
        size_bytes: size.parse::<u64>().unwrap_or(0),
    })
}

/// Check whether a normalized item should be visible to the user.
pub fn is_displayable_item(item: &NormalizedItem) -> bool {
    item.has_sub_tag
        && !item.title.contains("調整中")
        && item.size_bytes > MINIMUM_VISIBLE_SIZE_BYTES
}

/// Build a filtered list of normalized items suitable for display and asset-id lookup.
pub fn normalized_display_items(list_json: &Value) -> Result<Vec<NormalizedItem>> {
    let mut items = Vec::new();
    for item in iter_delivery_contents(list_json)? {
        let normalized = normalize_item(&item)?;
        if is_displayable_item(&normalized) {
            items.push(normalized);
        }
    }
    Ok(items)
}

/// Find a raw item by its asset id after applying the same display filters used by the CLI.
pub fn find_item_by_asset_id(list_json: &Value, asset_id: &str) -> Result<Value> {
    for item in iter_delivery_contents(list_json)? {
        let normalized = normalize_item(&item)?;
        if is_displayable_item(&normalized) && normalized.asset_id == asset_id {
            return Ok(item);
        }
    }
    Err(BuddyCastError::ItemNotFound(asset_id.to_string()).into())
}

/// Find a raw item by its item id.
pub fn find_item_by_id(list_json: &Value, item_id: &str) -> Result<Value> {
    for item in iter_delivery_contents(list_json)? {
        if item.get("id").and_then(Value::as_str) == Some(item_id) {
            return Ok(item);
        }
    }
    Err(BuddyCastError::ItemNotFound(item_id.to_string()).into())
}

/// Download the encrypted zip referenced by a raw item.
pub fn download_encrypted_zip(
    item: &Value,
    timeout: f64,
    retries: usize,
    retry_delay_seconds: f64,
) -> Result<Vec<u8>> {
    let normalized = normalize_item(item)?;
    retry_bytes(retries, retry_delay_seconds, || {
        let client = Client::builder()
            .timeout(Duration::from_secs_f64(timeout))
            .build()
            .context("failed to build HTTP client")?;
        let response = client
            .get(&normalized.url)
            .header("User-Agent", "UDCast/5.3.2 Rust port")
            .send()
            .with_context(|| format!("failed to download encrypted zip from {}", normalized.url))?
            .error_for_status()
            .context("encrypted zip returned an error status")?;
        let progress_handle = progress_bar_for_response_length(
            response.content_length(),
            &format!("Downloading {}...", normalized.asset_id),
        );
        read_response_bytes(response, progress_handle)
    })
}

fn read_response_bytes(
    mut response: reqwest::blocking::Response,
    progress_handle: ProgressHandle,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8 * 1024];

    loop {
        let read = response
            .read(&mut chunk)
            .context("failed while reading HTTP response body")?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
        progress_handle.inc(read as u64);
    }

    progress_handle.finish_and_clear();
    Ok(bytes)
}

fn progress_bar_for_response_length(content_length: Option<u64>, message: &str) -> ProgressHandle {
    match content_length {
        Some(total_bytes) => progress::bytes_bar(total_bytes, message),
        None => progress::spinner(message),
    }
}

fn has_sub_tag(tag: &Value) -> bool {
    tag.as_str()
        .map(|value| value.eq_ignore_ascii_case("sub"))
        .unwrap_or(false)
}

fn retry_bytes<F>(retries: usize, retry_delay_seconds: f64, mut operation: F) -> Result<Vec<u8>>
where
    F: FnMut() -> Result<Vec<u8>>,
{
    let attempts = retries.saturating_add(1).max(1);
    let mut last_error = None;

    for attempt in 0..attempts {
        match operation() {
            Ok(bytes) => return Ok(bytes),
            Err(error) => {
                last_error = Some(error);
                if attempt + 1 < attempts && retry_delay_seconds > 0.0 {
                    std::thread::sleep(Duration::from_secs_f64(retry_delay_seconds));
                }
            }
        }
    }

    match last_error {
        Some(error) => Err(error),
        None => unreachable!("retry loop must execute at least once"),
    }
}
