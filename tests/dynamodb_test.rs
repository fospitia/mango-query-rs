use mango_query_rs::{DynamoDBCompiler, FlavourCompiler};
use serde_json::json;

#[test]
fn test_translate_simple_equality() {
    let compiler = DynamoDBCompiler::new();
    let query = json!({ "name": "Juan", "status": "active" });
    let result = compiler.compile(&query, None).unwrap();

    // Since map order might differ in Rust, FilterExpression can be either way.
    // Let's assert parts or parse/sort them.
    let expr = result.filter_expression;
    assert!(
        expr == "#attr_name = :val_0 AND #attr_status = :val_1"
            || expr == "#attr_status = :val_0 AND #attr_name = :val_1"
    );

    assert_eq!(
        result.expression_attribute_names.get("#attr_name").unwrap(),
        "name"
    );
    assert_eq!(
        result
            .expression_attribute_names
            .get("#attr_status")
            .unwrap(),
        "status"
    );

    if expr.starts_with("#attr_name") {
        assert_eq!(
            result.expression_attribute_values.get(":val_0").unwrap(),
            &json!("Juan")
        );
        assert_eq!(
            result.expression_attribute_values.get(":val_1").unwrap(),
            &json!("active")
        );
    } else {
        assert_eq!(
            result.expression_attribute_values.get(":val_0").unwrap(),
            &json!("active")
        );
        assert_eq!(
            result.expression_attribute_values.get(":val_1").unwrap(),
            &json!("Juan")
        );
    }
}

#[test]
fn test_translate_explicit_comparison_operators() {
    let compiler = DynamoDBCompiler::new();
    let query = json!({ "age": { "$gte": 18, "$lte": 65 } });
    let result = compiler.compile(&query, None).unwrap();

    let expr = result.filter_expression;
    assert!(
        expr == "#attr_age >= :val_0 AND #attr_age <= :val_1"
            || expr == "#attr_age <= :val_0 AND #attr_age >= :val_1"
    );
    assert_eq!(
        result.expression_attribute_names.get("#attr_age").unwrap(),
        "age"
    );

    if expr.contains(">=") && expr.find(">=").unwrap() < expr.find("<=").unwrap() {
        assert_eq!(
            result.expression_attribute_values.get(":val_0").unwrap(),
            &json!(18)
        );
        assert_eq!(
            result.expression_attribute_values.get(":val_1").unwrap(),
            &json!(65)
        );
    } else {
        assert_eq!(
            result.expression_attribute_values.get(":val_0").unwrap(),
            &json!(65)
        );
        assert_eq!(
            result.expression_attribute_values.get(":val_1").unwrap(),
            &json!(18)
        );
    }
}

#[test]
fn test_translate_nested_properties() {
    let compiler = DynamoDBCompiler::new();
    let query_dot = json!({ "imdb.rating": 8 });
    let result_dot = compiler.compile(&query_dot, None).unwrap();
    assert_eq!(
        result_dot.filter_expression,
        "#attr_imdb.#attr_rating = :val_0"
    );
    assert_eq!(
        result_dot
            .expression_attribute_names
            .get("#attr_imdb")
            .unwrap(),
        "imdb"
    );
    assert_eq!(
        result_dot
            .expression_attribute_names
            .get("#attr_rating")
            .unwrap(),
        "rating"
    );

    let query_obj = json!({ "imdb": { "rating": 8 } });
    let result_obj = compiler.compile(&query_obj, None).unwrap();
    assert_eq!(
        result_obj.filter_expression,
        "#attr_imdb.#attr_rating = :val_0"
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
        result.filter_expression,
        "(#attr_name = :val_0 OR #attr_status <> :val_1)"
    );
    assert_eq!(
        result.expression_attribute_names.get("#attr_name").unwrap(),
        "name"
    );
    assert_eq!(
        result
            .expression_attribute_names
            .get("#attr_status")
            .unwrap(),
        "status"
    );
    assert_eq!(
        result.expression_attribute_values.get(":val_0").unwrap(),
        &json!("Juan")
    );
    assert_eq!(
        result.expression_attribute_values.get(":val_1").unwrap(),
        &json!("archived")
    );
}

#[test]
fn test_translate_special_functions() {
    let compiler = DynamoDBCompiler::new();

    let result_exists = compiler
        .compile(&json!({ "role": { "$exists": true } }), None)
        .unwrap();
    assert_eq!(
        result_exists.filter_expression,
        "attribute_exists(#attr_role)"
    );

    let result_not_exists = compiler
        .compile(&json!({ "role": { "$exists": false } }), None)
        .unwrap();
    assert_eq!(
        result_not_exists.filter_expression,
        "attribute_not_exists(#attr_role)"
    );

    let result_begins = compiler
        .compile(&json!({ "email": { "$beginsWith": "admin@" } }), None)
        .unwrap();
    assert_eq!(
        result_begins.filter_expression,
        "begins_with(#attr_email, :val_0)"
    );
    assert_eq!(
        result_begins
            .expression_attribute_values
            .get(":val_0")
            .unwrap(),
        &json!("admin@")
    );

    let result_type = compiler
        .compile(&json!({ "count": { "$type": "number" } }), None)
        .unwrap();
    assert_eq!(
        result_type.filter_expression,
        "attribute_type(#attr_count, :val_0)"
    );
    assert_eq!(
        result_type
            .expression_attribute_values
            .get(":val_0")
            .unwrap(),
        &json!("N")
    );
}

#[test]
fn test_translate_array_operators() {
    let compiler = DynamoDBCompiler::new();

    let query_in = json!({ "status": { "$in": ["active", "pending"] } });
    let result_in = compiler.compile(&query_in, None).unwrap();
    assert_eq!(
        result_in.filter_expression,
        "#attr_status IN (:val_0, :val_1)"
    );
    assert_eq!(
        result_in.expression_attribute_values.get(":val_0").unwrap(),
        &json!("active")
    );
    assert_eq!(
        result_in.expression_attribute_values.get(":val_1").unwrap(),
        &json!("pending")
    );

    let query_nin = json!({ "status": { "$nin": ["deleted"] } });
    let result_nin = compiler.compile(&query_nin, None).unwrap();
    assert_eq!(
        result_nin.filter_expression,
        "NOT (#attr_status IN (:val_0))"
    );
}
