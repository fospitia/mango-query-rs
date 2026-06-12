use mango_query_rs::{MangoQueryBuilder, SortRule, UseIndex};
use serde_json::json;
use std::collections::HashMap;

#[test]
fn test_build_empty_query() {
    let query = MangoQueryBuilder::new().build();
    assert_eq!(query.selector, json!({}));
}

#[test]
fn test_build_simple_query_implicit_equality() {
    let query = MangoQueryBuilder::new()
        .r#where("name", json!("Juan"))
        .build();

    assert_eq!(query.selector, json!({ "name": "Juan" }));
}

#[test]
fn test_build_query_multiple_conditions_on_same_field() {
    let query = MangoQueryBuilder::new()
        .where_op("age", "$gte", json!(18))
        .where_op("age", "$lte", json!(30))
        .build();

    assert_eq!(
        query.selector,
        json!({
            "age": {
                "$gte": 18,
                "$lte": 30
            }
        })
    );
}

#[test]
fn test_build_explicit_condition_operator_objects() {
    let query = MangoQueryBuilder::new()
        .r#where("age", json!({ "$gte": 18, "$lte": 30 }))
        .build();

    assert_eq!(
        query.selector,
        json!({
            "age": {
                "$gte": 18,
                "$lte": 30
            }
        })
    );
}

#[test]
fn test_build_logical_and_or_nor_not() {
    let query = MangoQueryBuilder::new()
        .r#where("year", json!(1977))
        .or(vec![
            json!({ "director": "George Lucas" }),
            json!({ "director": "Steven Spielberg" }),
        ])
        .build();

    assert_eq!(
        query.selector,
        json!({
            "year": 1977,
            "$or": [
                { "director": "George Lucas" },
                { "director": "Steven Spielberg" }
            ]
        })
    );
}

#[test]
fn test_specify_text_queries() {
    let query = MangoQueryBuilder::new().text("director:George").build();

    assert_eq!(
        query.selector,
        json!({
            "$text": "director:George"
        })
    );
}

#[test]
fn test_build_query_metadata_fields_sort_limit_skip_use_index() {
    let query = MangoQueryBuilder::new()
        .r#where("status", json!("active"))
        .fields(vec!["name".to_string(), "age".to_string()])
        .sort(vec![SortRule::Field("name".to_string())])
        .limit(10)
        .skip(5)
        .use_index(UseIndex::DesignDoc("status-index".to_string()))
        .build();

    assert_eq!(query.selector, json!({ "status": "active" }));
    assert_eq!(
        query.fields,
        Some(vec!["name".to_string(), "age".to_string()])
    );
    assert_eq!(query.sort, Some(vec![SortRule::Field("name".to_string())]));
    assert_eq!(query.limit, Some(10));
    assert_eq!(query.skip, Some(5));
    assert_eq!(
        query.use_index,
        Some(UseIndex::DesignDoc("status-index".to_string()))
    );
}

#[test]
fn test_sort_chaining_and_direction_defaults() {
    let query = MangoQueryBuilder::new()
        .sort_field("year", Some("desc"))
        .sort_field("title", None)
        .build();

    let mut expected_map = HashMap::new();
    expected_map.insert("year".to_string(), "desc".to_string());

    assert_eq!(
        query.sort,
        Some(vec![
            SortRule::FieldDirection(expected_map),
            SortRule::Field("title".to_string())
        ])
    );
}

#[test]
fn test_additional_find_parameters() {
    let query = MangoQueryBuilder::new()
        .r#where("status", json!("active"))
        .allow_fallback(false)
        .conflicts(true)
        .r(2)
        .stable(true)
        .build();

    assert_eq!(query.allow_fallback, Some(false));
    assert_eq!(query.conflicts, Some(true));
    assert_eq!(query.r, Some(2));
    assert_eq!(query.stable, Some(true));
}

#[test]
fn test_pagination_bookmark_update_execution_stats() {
    let query = MangoQueryBuilder::new()
        .r#where("status", json!("active"))
        .bookmark("g1AAAAD-eJzLYWBgYMpgSmHgKy5JLCrJTq2MT8lPzkzJDEGQAwom5ycn5ucl5gAEg...")
        .update(false)
        .execution_stats(true)
        .build();

    assert_eq!(
        query.bookmark,
        Some("g1AAAAD-eJzLYWBgYMpgSmHgKy5JLCrJTq2MT8lPzkzJDEGQAwom5ycn5ucl5gAEg...".to_string())
    );
    assert_eq!(query.update, Some(false));
    assert_eq!(query.execution_stats, Some(true));
}
