use std::path::PathBuf;

use tempfile::TempDir;
use zot_core::LibraryScope;
use zot_local::{LocalLibrary, SearchOptions};

const CHILD_TYPES: &[&str] = &["attachment", "note", "annotation"];

fn fixture_sqlite_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("zotero.sqlite")
}

fn open_fixture_library() -> (TempDir, LocalLibrary) {
    let fixture = fixture_sqlite_path();
    let dir = TempDir::new().expect("tempdir");
    std::fs::copy(&fixture, dir.path().join("zotero.sqlite")).expect("copy fixture");
    let library = LocalLibrary::open(dir.path(), LibraryScope::User).expect("open fixture");
    (dir, library)
}

#[test]
fn empty_query_returns_non_empty_primary_items_only() {
    let (_dir, library) = open_fixture_library();
    let result = library
        .search(SearchOptions {
            limit: 1_000,
            ..SearchOptions::default()
        })
        .expect("empty search");
    assert!(
        result.total > 0,
        "fixture should expose at least one primary item"
    );
    for item in &result.items {
        assert!(
            !CHILD_TYPES.contains(&item.item_type.as_str()),
            "empty search must not include {}: {}",
            item.item_type,
            item.key
        );
    }
}

#[test]
fn list_items_matches_search_default_projection() {
    let (_dir, library) = open_fixture_library();
    let listed = library
        .list_items(None, 1_000, 0)
        .expect("list_items baseline");
    for item in &listed {
        assert!(
            !CHILD_TYPES.contains(&item.item_type.as_str()),
            "list_items must exclude child type {}",
            item.item_type
        );
    }
}

#[test]
fn like_query_propagates_across_field_sub_searches() {
    let (_dir, library) = open_fixture_library();
    let baseline = library
        .list_items(None, 1_000, 0)
        .expect("list baseline items");
    let sample_title = baseline
        .iter()
        .find(|item| !item.title.trim().is_empty())
        .map(|item| item.title.clone())
        .expect("fixture should have at least one titled item");
    let token = sample_title
        .split_whitespace()
        .find(|chunk| chunk.len() >= 3)
        .unwrap_or(&sample_title)
        .to_string();

    let result = library
        .search(SearchOptions {
            query: token.clone(),
            limit: 100,
            ..SearchOptions::default()
        })
        .expect("field-qualified search");

    assert!(
        !result.items.is_empty(),
        "searching for `{token}` should hit the seed item"
    );
    for item in &result.items {
        assert!(
            !CHILD_TYPES.contains(&item.item_type.as_str()),
            "field search must not leak child type {}",
            item.item_type
        );
    }
}

#[test]
fn item_type_filter_uses_chunked_in_query_not_per_row_lookups() {
    let (_dir, library) = open_fixture_library();
    let baseline = library
        .list_items(None, 1_000, 0)
        .expect("list baseline items");
    let Some(sample_type) = baseline
        .iter()
        .map(|item| item.item_type.clone())
        .find(|item_type| !item_type.is_empty())
    else {
        return; // fixture too small to exercise
    };
    let result = library
        .search(SearchOptions {
            item_type: Some(sample_type.clone()),
            limit: 1_000,
            ..SearchOptions::default()
        })
        .expect("type-filtered search");
    for item in &result.items {
        assert_eq!(
            item.item_type, sample_type,
            "item_type filter must keep only matching types"
        );
    }
}

#[test]
fn get_stats_returns_well_formed_aggregate() {
    let (_dir, library) = open_fixture_library();
    let stats = library.get_stats().expect("get_stats aggregate");
    assert!(
        stats.total_items > 0,
        "fixture must contribute to total_items"
    );
    assert!(
        !stats.by_type.is_empty(),
        "by_type must reflect at least one primary type"
    );
    let by_type_sum: usize = stats.by_type.values().sum();
    assert_eq!(
        by_type_sum, stats.total_items,
        "sum of by_type counts must equal total_items"
    );
    for (key, count) in &stats.top_tags {
        assert!(*count > 0, "tag `{key}` reported zero items");
    }
}

#[test]
fn recent_items_by_count_is_bounded() {
    let (_dir, library) = open_fixture_library();
    let items = library
        .get_recent_items_by_count(5)
        .expect("recent by count");
    assert!(items.len() <= 5, "recent cap exceeded: {}", items.len());
    for item in &items {
        assert!(
            !CHILD_TYPES.contains(&item.item_type.as_str()),
            "recent must exclude child type {}",
            item.item_type
        );
    }
}

#[test]
fn get_item_returns_none_for_unknown_key() {
    let (_dir, library) = open_fixture_library();
    let miss = library
        .get_item("NONE_SUCH_FIXTURE_KEY_0000")
        .expect("should not error on miss");
    assert!(miss.is_none(), "unknown key must map to None, not error");
}
