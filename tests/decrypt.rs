use buddy_cast::decrypt::{decrypt_bytes, is_zip_bytes};

#[test]
fn decrypt_zip_matches_expected_bytes() {
    let encrypted = std::fs::read("fixtures/encrypted.zip").expect("fixture should exist");
    let expected = std::fs::read("fixtures/decrypted.zip").expect("fixture should exist");

    let actual = decrypt_bytes(&encrypted);

    assert_eq!(actual, expected);
    assert!(is_zip_bytes(&actual));
}
