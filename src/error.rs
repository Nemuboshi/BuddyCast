use std::path::PathBuf;

use thiserror::Error;

/// Domain-specific failures that can occur while handling UDCast data.
#[derive(Debug, Error)]
pub enum BuddyCastError {
    #[error("expected JSON path contents.deliveryContents")]
    MissingDeliveryContents,
    #[error("contents.deliveryContents is not a list")]
    DeliveryContentsNotList,
    #[error("missing item.contents object")]
    MissingItemContents,
    #[error("missing item.id")]
    MissingItemId,
    #[error("missing item.contents.eTags")]
    MissingAssetId,
    #[error("missing item.contents.url")]
    MissingItemUrl,
    #[error("item id not found: {0}")]
    ItemNotFound(String),
    #[error("decrypted bytes are not a zip archive")]
    InvalidDecryptedZip,
    #[error("subtitle xml parse failed: {0}")]
    SubtitleParse(String),
    #[error("database file is too small to be a UDCast MFCC db: {path}")]
    DbTooSmall { path: String },
    #[error("database file ended while reading {field} at offset {offset}")]
    DbTruncated { field: &'static str, offset: usize },
    #[error("offline fetch failed: contents json not found: {0}")]
    OfflineListMissing(PathBuf),
    #[error("offline fetch failed: encrypted package not found: {0}")]
    OfflinePackageMissing(PathBuf),
}
