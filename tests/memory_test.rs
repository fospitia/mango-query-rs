use base64::{Engine as _, engine::general_purpose::STANDARD};
use mango_query_rs::{InMemoryFilter, InMemoryFilterOptions, MangoQueryBuilder, SortRule};
use serde_json::{Value, json};
use std::collections::HashMap;

#[test]
fn test_matches_implicit_equality() {
    let item = json!({ "name": "Juan", "age": 30 });

    let query_match = json!({ "name": "Juan" });
    assert!(InMemoryFilter::matches(&item, &query_match));

    let query_no_match = json!({ "name": "Pedro" });
    assert!(!InMemoryFilter::matches(&item, &query_no_match));
}

#[test]
fn test_matches_nested_fields_dot_notation_and_objects() {
    let item = json!({ "imdb": { "rating": 8.5, "votes": 1000 } });

    assert!(InMemoryFilter::matches(
        &item,
        &json!({ "imdb.rating": 8.5 })
    ));
    assert!(InMemoryFilter::matches(
        &item,
        &json!({ "imdb": { "rating": 8.5 } })
    ));
    assert!(InMemoryFilter::matches(
        &item,
        &json!({ "imdb": { "rating": { "$gte": 8.0 } } })
    ));
}

#[test]
fn test_matches_comparison_and_operators() {
    let item = json!({ "age": 30, "tags": ["js", "ts"], "code": 10 });

    assert!(InMemoryFilter::matches(
        &item,
        &json!({ "age": { "$gt": 20, "$lte": 30 } })
    ));
    assert!(InMemoryFilter::matches(
        &item,
        &json!({ "age": { "$type": "number" } })
    ));
    assert!(InMemoryFilter::matches(
        &item,
        &json!({ "code": { "$mod": [5, 0] } })
    ));
    assert!(InMemoryFilter::matches(
        &item,
        &json!({ "tags": { "$in": ["ts"] } })
    ));
    assert!(InMemoryFilter::matches(
        &item,
        &json!({ "tags": { "$size": 2 } })
    ));
}

fn get_mock_data() -> Vec<Value> {
    vec![
        json!({ "tenantId": "t1", "id": "101", "name": "A", "category": "high", "age": 25 }),
        json!({ "tenantId": "t1", "id": "102", "name": "B", "category": "low", "age": 40 }),
        json!({ "tenantId": "t1", "id": "103", "name": "A", "category": "medium", "age": 30 }),
        json!({ "tenantId": "t1", "id": "104", "name": "C", "category": "low", "age": 18 }),
        json!({ "tenantId": "t1", "id": "105", "name": "A", "category": "high", "age": 35 }),
        json!({ "tenantId": "t1", "id": "106", "name": "A", "category": "low", "age": 20 }),
    ]
}

#[test]
fn test_filter_and_paginate_single_pk() {
    let data = get_mock_data();
    let query = MangoQueryBuilder::new().r#where("name", json!("A")).build();
    let pk = vec!["id".to_string()];

    // Page 1: Limit = 2
    let options = InMemoryFilterOptions {
        data: &data,
        query: &query,
        pk: &pk,
        limit: Some(2),
        bookmark: None,
    };
    let result1 = InMemoryFilter::filter(options).unwrap();
    assert_eq!(result1.docs.len(), 2);
    assert_eq!(result1.docs[0]["id"], "101");
    assert_eq!(result1.docs[1]["id"], "103");
    assert!(result1.bookmark.is_some());

    // Decode bookmark to verify
    let bookmark_str = result1.bookmark.as_ref().unwrap();
    let decoded_bytes = STANDARD.decode(bookmark_str).unwrap();
    let decoded_map: HashMap<String, Value> = serde_json::from_slice(&decoded_bytes).unwrap();
    assert_eq!(decoded_map.get("id").unwrap(), "103");

    // Page 2: Resume
    let options2 = InMemoryFilterOptions {
        data: &data,
        query: &query,
        pk: &pk,
        limit: Some(2),
        bookmark: Some(bookmark_str),
    };
    let result2 = InMemoryFilter::filter(options2).unwrap();
    assert_eq!(result2.docs.len(), 2);
    assert_eq!(result2.docs[0]["id"], "105");
    assert_eq!(result2.docs[1]["id"], "106");
    assert!(result2.bookmark.is_none());
}

#[test]
fn test_filter_and_paginate_composite_pk() {
    let data = get_mock_data();
    let query = MangoQueryBuilder::new().r#where("name", json!("A")).build();
    let pk = vec!["tenantId".to_string(), "id".to_string()];

    // Page 1: Limit = 1
    let options1 = InMemoryFilterOptions {
        data: &data,
        query: &query,
        pk: &pk,
        limit: Some(1),
        bookmark: None,
    };
    let result1 = InMemoryFilter::filter(options1).unwrap();
    assert_eq!(result1.docs.len(), 1);
    assert_eq!(result1.docs[0]["id"], "101");
    assert!(result1.bookmark.is_some());

    let bookmark_str = result1.bookmark.as_ref().unwrap();
    let decoded_bytes = STANDARD.decode(bookmark_str).unwrap();
    let decoded_map: HashMap<String, Value> = serde_json::from_slice(&decoded_bytes).unwrap();
    assert_eq!(decoded_map.get("tenantId").unwrap(), "t1");
    assert_eq!(decoded_map.get("id").unwrap(), "101");

    // Page 2: Limit = 2, Resume
    let options2 = InMemoryFilterOptions {
        data: &data,
        query: &query,
        pk: &pk,
        limit: Some(2),
        bookmark: Some(bookmark_str),
    };
    let result2 = InMemoryFilter::filter(options2).unwrap();
    assert_eq!(result2.docs.len(), 2);
    assert_eq!(result2.docs[0]["id"], "103");
    assert_eq!(result2.docs[1]["id"], "105");
    assert!(result2.bookmark.is_some());
}

#[test]
fn test_apply_sorting_rules() {
    let data = get_mock_data();
    let query = MangoQueryBuilder::new()
        .r#where("name", json!("A"))
        .sort(vec![SortRule::Field("age".to_string())])
        .build();
    let pk = vec!["id".to_string()];

    // Sorted age: 20 (id 106), 25 (id 101), 30 (id 103), 35 (id 105)
    // Page 1: Limit = 3
    let options1 = InMemoryFilterOptions {
        data: &data,
        query: &query,
        pk: &pk,
        limit: Some(3),
        bookmark: None,
    };
    let result1 = InMemoryFilter::filter(options1).unwrap();
    assert_eq!(result1.docs.len(), 3);
    assert_eq!(result1.docs[0]["id"], "106");
    assert_eq!(result1.docs[1]["id"], "101");
    assert_eq!(result1.docs[2]["id"], "103");
    assert!(result1.bookmark.is_some());

    let bookmark_str = result1.bookmark.as_ref().unwrap();
    let decoded_bytes = STANDARD.decode(bookmark_str).unwrap();
    let decoded_map: HashMap<String, Value> = serde_json::from_slice(&decoded_bytes).unwrap();
    assert_eq!(decoded_map.get("id").unwrap(), "103");

    // Page 2: Limit = 3, Resume
    let options2 = InMemoryFilterOptions {
        data: &data,
        query: &query,
        pk: &pk,
        limit: Some(3),
        bookmark: Some(bookmark_str),
    };
    let result2 = InMemoryFilter::filter(options2).unwrap();
    assert_eq!(result2.docs.len(), 1);
    assert_eq!(result2.docs[0]["id"], "105");
    assert!(result2.bookmark.is_none());
}

#[test]
fn test_apply_fields_projection() {
    let data = get_mock_data();
    let query = MangoQueryBuilder::new()
        .r#where("name", json!("A"))
        .fields(vec!["id".to_string(), "category".to_string()])
        .build();
    let pk = vec!["id".to_string()];

    let options = InMemoryFilterOptions {
        data: &data,
        query: &query,
        pk: &pk,
        limit: Some(2),
        bookmark: None,
    };
    let result = InMemoryFilter::filter(options).unwrap();
    assert_eq!(result.docs.len(), 2);

    assert_eq!(result.docs[0], json!({ "id": "101", "category": "high" }));
    assert_eq!(result.docs[1], json!({ "id": "103", "category": "medium" }));
}
