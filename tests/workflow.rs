use buddy_cast::workflow::{
    OfflineRequest, WorkflowOptions, fetch_offline_by_asset_id, process_local_encrypted_zip,
};
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn workflow_processes_local_encrypted_zip() {
    let temp_dir = tempdir().expect("temp dir should be created");
    let result = process_local_encrypted_zip(
        std::path::Path::new("fixtures/encrypted.zip"),
        temp_dir.path(),
        WorkflowOptions {
            export_ass: true,
            export_srt: false,
            parse_db_files: true,
            keep_encrypted_zip: true,
        },
    )
    .expect("workflow should succeed");

    let decrypted_zip_path = result
        .decrypted_zip_path
        .expect("decrypted zip path should exist");
    assert_eq!(
        std::fs::read(&decrypted_zip_path).expect("decrypted zip should exist"),
        std::fs::read("fixtures/decrypted.zip").expect("fixture should exist")
    );

    assert_eq!(
        result
            .subtitle_outputs
            .iter()
            .map(|path| path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string())
            .collect::<Vec<_>>(),
        vec!["subtitles_ja.ass".to_string()]
    );
    assert_eq!(
        std::fs::read_to_string(&result.subtitle_outputs[0]).expect("subtitle output should exist"),
        std::fs::read_to_string("fixtures/expected.ass").expect("fixture should exist")
    );

    assert_eq!(
        result
            .db_reports
            .iter()
            .map(|path| path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string())
            .collect::<Vec<_>>(),
        vec!["sample.db.json".to_string()]
    );
    let actual_db: Value = serde_json::from_str(
        &std::fs::read_to_string(&result.db_reports[0]).expect("db report should exist"),
    )
    .expect("actual db report should parse");
    let expected_db: Value = serde_json::from_str(
        &std::fs::read_to_string("fixtures/expected_db.json").expect("fixture should exist"),
    )
    .expect("expected db report should parse");
    assert_eq!(actual_db, expected_db);
}

#[test]
fn offline_workflow_copies_contents_uses_title_directory_and_removes_encrypted_zip() {
    let temp_dir = tempdir().expect("temp dir should be created");
    let result = fetch_offline_by_asset_id(
        &OfflineRequest {
            asset_id: "sample-asset-1".to_string(),
            list_json_path: std::path::PathBuf::from("fixtures/contents.sample.json"),
            encrypted_zip_path: std::path::PathBuf::from("fixtures/encrypted.zip"),
        },
        temp_dir.path(),
        WorkflowOptions {
            export_ass: true,
            export_srt: true,
            parse_db_files: true,
            keep_encrypted_zip: false,
        },
    )
    .expect("offline workflow should succeed");

    assert!(temp_dir.path().join("contents.json").exists());
    assert!(result.encrypted_zip_path.is_none());
    let extract_dir = result.extract_dir.expect("extract dir should exist");
    assert_eq!(
        extract_dir.file_name().and_then(|name| name.to_str()),
        Some("Sample Title")
    );
    assert_eq!(result.subtitle_outputs.len(), 2);
}

#[test]
fn offline_workflow_fails_for_missing_item_id() {
    let temp_dir = tempdir().expect("temp dir should be created");
    let error = fetch_offline_by_asset_id(
        &OfflineRequest {
            asset_id: "missing-asset".to_string(),
            list_json_path: std::path::PathBuf::from("fixtures/contents.sample.json"),
            encrypted_zip_path: std::path::PathBuf::from("fixtures/encrypted.zip"),
        },
        temp_dir.path(),
        WorkflowOptions {
            export_ass: true,
            export_srt: false,
            parse_db_files: false,
            keep_encrypted_zip: true,
        },
    )
    .expect_err("missing item should fail");
    assert!(
        error.to_string().contains("item id not found")
            || error.to_string().contains("missing-asset")
    );
}
