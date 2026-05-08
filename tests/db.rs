use buddy_cast::db::{parse_db_bytes, parse_db_file};
use serde_json::Value;

#[test]
fn db_parse_matches_expected() {
    let actual = parse_db_file(
        std::path::Path::new("fixtures/sample.db"),
        Some("sample.db"),
    )
    .expect("db fixture should parse");
    let expected: Value = serde_json::from_str(
        &std::fs::read_to_string("fixtures/expected_db.json").expect("fixture should exist"),
    )
    .expect("expected json should parse");
    assert_eq!(actual, expected);
}

#[test]
fn db_parse_rejects_truncated_input() {
    let error = parse_db_bytes(&[0_u8; 10], "tiny.db").expect_err("small db should fail");
    assert!(error.to_string().contains("too small"));
}
