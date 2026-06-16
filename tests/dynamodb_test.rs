#![cfg(feature = "dynamodb")]

use aws_sdk_dynamodb::types::AttributeValue;
use mango_query_rs::{DynamoDBCompiler, DynamoDBConfig, FlavourCompiler};
use serde_json::json;

#[test]
fn test_translate_simple_equality() {
    let compiler = DynamoDBCompiler::new();
    let query = json!({ "name": "Juan", "status": "active" });
    let result = compiler.compile(&query, None).unwrap();

    // Since map order might differ in Rust, FilterExpression can be either way.
    // Let's assert parts or parse/sort them.
    let expr = result
        .filter_expression
        .expect("Expected filter expression");
    assert!(
        expr == "#attr_name = :val_0 AND #attr_status = :val_1"
            || expr == "#attr_status = :val_0 AND #attr_name = :val_1"
    );

    let attr_names = result
        .expression_attribute_names
        .expect("Expected attribute names");
    assert_eq!(attr_names.get("#attr_name").unwrap(), "name");
    assert_eq!(attr_names.get("#attr_status").unwrap(), "status");

    let attr_values = result
        .expression_attribute_values
        .expect("Expected attribute values");
    if expr.starts_with("#attr_name") {
        assert_eq!(
            attr_values.get(":val_0").unwrap(),
            &AttributeValue::S("Juan".to_string())
        );
        assert_eq!(
            attr_values.get(":val_1").unwrap(),
            &AttributeValue::S("active".to_string())
        );
    } else {
        assert_eq!(
            attr_values.get(":val_0").unwrap(),
            &AttributeValue::S("active".to_string())
        );
        assert_eq!(
            attr_values.get(":val_1").unwrap(),
            &AttributeValue::S("Juan".to_string())
        );
    }
}

#[test]
fn test_translate_explicit_comparison_operators() {
    let compiler = DynamoDBCompiler::new();
    let query = json!({ "age": { "$gte": 18, "$lte": 65 } });
    let result = compiler.compile(&query, None).unwrap();

    let expr = result
        .filter_expression
        .expect("Expected filter expression");
    assert!(
        expr == "#attr_age >= :val_0 AND #attr_age <= :val_1"
            || expr == "#attr_age <= :val_0 AND #attr_age >= :val_1"
    );
    let attr_names = result
        .expression_attribute_names
        .expect("Expected attribute names");
    assert_eq!(attr_names.get("#attr_age").unwrap(), "age");

    let attr_values = result
        .expression_attribute_values
        .expect("Expected attribute values");
    if expr.contains(">=") && expr.find(">=").unwrap() < expr.find("<=").unwrap() {
        assert_eq!(
            attr_values.get(":val_0").unwrap(),
            &AttributeValue::N("18".to_string())
        );
        assert_eq!(
            attr_values.get(":val_1").unwrap(),
            &AttributeValue::N("65".to_string())
        );
    } else {
        assert_eq!(
            attr_values.get(":val_0").unwrap(),
            &AttributeValue::N("65".to_string())
        );
        assert_eq!(
            attr_values.get(":val_1").unwrap(),
            &AttributeValue::N("18".to_string())
        );
    }
}

#[test]
fn test_translate_nested_properties() {
    let compiler = DynamoDBCompiler::new();
    let query_dot = json!({ "imdb.rating": 8 });
    let result_dot = compiler.compile(&query_dot, None).unwrap();
    assert_eq!(
        result_dot.filter_expression.as_deref(),
        Some("#attr_imdb.#attr_rating = :val_0")
    );
    let attr_names_dot = result_dot
        .expression_attribute_names
        .expect("Expected attribute names");
    assert_eq!(attr_names_dot.get("#attr_imdb").unwrap(), "imdb");
    assert_eq!(attr_names_dot.get("#attr_rating").unwrap(), "rating");

    let query_obj = json!({ "imdb": { "rating": 8 } });
    let result_obj = compiler.compile(&query_obj, None).unwrap();
    assert_eq!(
        result_obj.filter_expression.as_deref(),
        Some("#attr_imdb.#attr_rating = :val_0")
    );
}

#[test]
fn test_translate_logical_combinations() {
    let compiler = DynamoDBCompiler::new();
    let query = json!({
        "$or": [
            { "name": "Juan" },
            { "status": { "$ne": "archived" } }
        ]
    });
    let result = compiler.compile(&query, None).unwrap();

    assert_eq!(
        result.filter_expression.as_deref(),
        Some("(#attr_name = :val_0 OR #attr_status <> :val_1)")
    );
    let attr_names = result
        .expression_attribute_names
        .expect("Expected attribute names");
    assert_eq!(attr_names.get("#attr_name").unwrap(), "name");
    assert_eq!(attr_names.get("#attr_status").unwrap(), "status");
    let attr_values = result
        .expression_attribute_values
        .expect("Expected attribute values");
    assert_eq!(
        attr_values.get(":val_0").unwrap(),
        &AttributeValue::S("Juan".to_string())
    );
    assert_eq!(
        attr_values.get(":val_1").unwrap(),
        &AttributeValue::S("archived".to_string())
    );
}

#[test]
fn test_translate_special_functions() {
    let compiler = DynamoDBCompiler::new();

    let result_exists = compiler
        .compile(&json!({ "role": { "$exists": true } }), None)
        .unwrap();
    assert_eq!(
        result_exists.filter_expression.as_deref(),
        Some("attribute_exists(#attr_role)")
    );

    let result_not_exists = compiler
        .compile(&json!({ "role": { "$exists": false } }), None)
        .unwrap();
    assert_eq!(
        result_not_exists.filter_expression.as_deref(),
        Some("attribute_not_exists(#attr_role)")
    );

    let result_begins = compiler
        .compile(&json!({ "email": { "$beginsWith": "admin@" } }), None)
        .unwrap();
    assert_eq!(
        result_begins.filter_expression.as_deref(),
        Some("begins_with(#attr_email, :val_0)")
    );
    assert_eq!(
        result_begins
            .expression_attribute_values
            .as_ref()
            .unwrap()
            .get(":val_0")
            .unwrap(),
        &AttributeValue::S("admin@".to_string())
    );

    let result_type = compiler
        .compile(&json!({ "count": { "$type": "number" } }), None)
        .unwrap();
    assert_eq!(
        result_type.filter_expression.as_deref(),
        Some("attribute_type(#attr_count, :val_0)")
    );
    assert_eq!(
        result_type
            .expression_attribute_values
            .as_ref()
            .unwrap()
            .get(":val_0")
            .unwrap(),
        &AttributeValue::S("N".to_string())
    );
}

#[test]
fn test_translate_array_operators() {
    let compiler = DynamoDBCompiler::new();

    let query_in = json!({ "status": { "$in": ["active", "pending"] } });
    let result_in = compiler.compile(&query_in, None).unwrap();
    assert_eq!(
        result_in.filter_expression.as_deref(),
        Some("#attr_status IN (:val_0, :val_1)")
    );
    assert_eq!(
        result_in
            .expression_attribute_values
            .as_ref()
            .unwrap()
            .get(":val_0")
            .unwrap(),
        &AttributeValue::S("active".to_string())
    );
    assert_eq!(
        result_in
            .expression_attribute_values
            .as_ref()
            .unwrap()
            .get(":val_1")
            .unwrap(),
        &AttributeValue::S("pending".to_string())
    );

    let query_nin = json!({ "status": { "$nin": ["deleted"] } });
    let result_nin = compiler.compile(&query_nin, None).unwrap();
    assert_eq!(
        result_nin.filter_expression.as_deref(),
        Some("NOT (#attr_status IN (:val_0))")
    );
}

#[test]
fn test_translate_empty_query() {
    let compiler = DynamoDBCompiler::new();
    let query = json!({});
    let result = compiler.compile(&query, None).unwrap();
    assert_eq!(result.key_condition, "");
    assert!(result.filter_expression.is_none());
    assert!(result.expression_attribute_names.is_none());
    assert!(result.expression_attribute_values.is_none());
}

#[test]
fn test_translate_with_dynamodb_config() {
    let compiler = DynamoDBCompiler::new();
    let query = json!({ "status": "active" });

    let mut attribute_names = std::collections::HashMap::new();
    attribute_names.insert("#initial_name".to_string(), "initial_val".to_string());

    let mut attribute_values = std::collections::HashMap::new();
    attribute_values.insert(
        ":initial_value".to_string(),
        AttributeValue::S("initial".to_string()),
    );

    let config = DynamoDBConfig {
        key_condition: "pk = :pk_val".to_string(),
        attribute_names,
        attribute_values,
    };

    let result = compiler.compile(&query, Some(config)).unwrap();

    assert_eq!(result.key_condition, "pk = :pk_val");
    assert_eq!(
        result.filter_expression.as_deref(),
        Some("#attr_status = :val_0")
    );

    let names = result.expression_attribute_names.unwrap();
    assert_eq!(names.get("#initial_name").unwrap(), "initial_val");
    assert_eq!(names.get("#attr_status").unwrap(), "status");

    let values = result.expression_attribute_values.unwrap();
    assert_eq!(
        values.get(":initial_value").unwrap(),
        &AttributeValue::S("initial".to_string())
    );
    assert_eq!(
        values.get(":val_0").unwrap(),
        &AttributeValue::S("active".to_string())
    );
}

#[test]
fn test_translate_with_index_and_bookmark() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let compiler = DynamoDBCompiler::new();

    // Test 1: use_index, bookmark, and limit in query
    let bmark_json = json!({ "id": "123", "score": 450 });
    let bmark_str = STANDARD.encode(serde_json::to_string(&bmark_json).unwrap());

    let query = json!({
        "selector": { "status": "active" },
        "use_index": "gsi_status_score",
        "bookmark": bmark_str,
        "limit": 10,
    });

    let result = compiler.compile(&query, None).unwrap();
    assert_eq!(result.index_name.as_deref(), Some("gsi_status_score"));
    assert_eq!(result.limit, Some(10));

    let exclusive_start = result.exclusive_start_key.unwrap();
    assert_eq!(
        exclusive_start.get("id").unwrap(),
        &AttributeValue::S("123".to_string())
    );
    assert_eq!(
        exclusive_start.get("score").unwrap(),
        &AttributeValue::N("450".to_string())
    );

    // Test 2: use_index as Array (first value is the design doc/index name)
    let query_arr = json!({
        "selector": { "status": "active" },
        "use_index": ["design_doc_name", "gsi_status_score"],
    });
    let result_arr = compiler.compile(&query_arr, None).unwrap();
    assert_eq!(result_arr.index_name.as_deref(), Some("design_doc_name"));
    assert_eq!(result_arr.limit, None);
}
