use buddy_cast::api::{
    find_item_by_asset_id, find_item_by_id, load_content_list, normalize_item,
    normalized_display_items,
};

#[test]
fn normalize_sample_item_matches_expected_fields() {
    let list_json = load_content_list(std::path::Path::new("fixtures/contents.sample.json"))
        .expect("sample contents should load");
    let raw_item = find_item_by_id(&list_json, "sample-item-1").expect("sample item should exist");
    let item = normalize_item(&raw_item).expect("sample item should normalize");

    assert_eq!(item.item_id, "sample-item-1");
    assert_eq!(item.asset_id, "sample-asset-1");
    assert_eq!(
        item.url,
        "https://cms.palabra.jp/masc/contentzip/sample-asset-1.zip"
    );
    assert_eq!(item.size, "123456");
    assert_eq!(item.size_bytes, 123456);
    assert_eq!(item.title, "Sample Title");
    assert!(item.has_sub_tag);
}

#[test]
fn display_items_apply_sub_title_and_size_filters() {
    let list_json = load_content_list(std::path::Path::new("fixtures/contents.sample.json"))
        .expect("sample contents should load");
    let items = normalized_display_items(&list_json).expect("filtered items should load");

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].asset_id, "sample-asset-1");
}

#[test]
fn find_item_by_asset_id_uses_filtered_set() {
    let list_json = load_content_list(std::path::Path::new("fixtures/contents.sample.json"))
        .expect("sample contents should load");

    assert!(find_item_by_asset_id(&list_json, "sample-asset-1").is_ok());
    assert!(find_item_by_asset_id(&list_json, "sample-asset-no-sub").is_err());
    assert!(find_item_by_asset_id(&list_json, "sample-asset-adjusting").is_err());
    assert!(find_item_by_asset_id(&list_json, "sample-asset-small").is_err());
}
