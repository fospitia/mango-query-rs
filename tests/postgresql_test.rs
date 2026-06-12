use mango_query_rs::{
    FlavourCompiler, PostgreSQLCompiler, PostgreSQLConfig, PostgresColumnConfig, PostgresJoinConfig,
};
use serde_json::json;
use std::collections::HashMap;

#[test]
fn test_translate_simple_equality_without_config() {
    let compiler = PostgreSQLCompiler::new();
    let query = json!({ "name": "Juan", "status": "active" });
    let result = compiler.compile(&query, None).unwrap();

    let clause = result.where_clause;
    assert!(clause == "name = $1 AND status = $2" || clause == "status = $1 AND name = $2");

    if clause.starts_with("name") {
        assert_eq!(result.values, vec![json!("Juan"), json!("active")]);
    } else {
        assert_eq!(result.values, vec![json!("active"), json!("Juan")]);
    }
    assert_eq!(result.joins, Vec::<String>::new());
}

#[test]
fn test_translate_column_mappings() {
    let compiler = PostgreSQLCompiler::new();
    let query = json!({ "name": "Juan", "status": "active" });

    let mut mappings = HashMap::new();
    mappings.insert(
        "name".to_string(),
        PostgresColumnConfig {
            table: "users".to_string(),
            column: "first_name".to_string(),
        },
    );
    mappings.insert(
        "status".to_string(),
        PostgresColumnConfig {
            table: "users".to_string(),
            column: "user_status".to_string(),
        },
    );

    let config = PostgreSQLConfig {
        column_mappings: mappings,
        joins: vec![],
        placeholder_start_index: None,
    };

    let result = compiler.compile(&query, Some(config)).unwrap();

    let clause = result.where_clause;
    assert!(
        clause == "users.first_name = $1 AND users.user_status = $2"
            || clause == "users.user_status = $1 AND users.first_name = $2"
    );

    if clause.starts_with("users.first_name") {
        assert_eq!(result.values, vec![json!("Juan"), json!("active")]);
    } else {
        assert_eq!(result.values, vec![json!("active"), json!("Juan")]);
    }
    assert_eq!(result.joins, Vec::<String>::new());
}

#[test]
fn test_generate_join_statements_dynamically() {
    let compiler = PostgreSQLCompiler::new();

    let mut mappings = HashMap::new();
    mappings.insert(
        "name".to_string(),
        PostgresColumnConfig {
            table: "users".to_string(),
            column: "first_name".to_string(),
        },
    );
    mappings.insert(
        "total".to_string(),
        PostgresColumnConfig {
            table: "orders".to_string(),
            column: "order_total".to_string(),
        },
    );

    let config = PostgreSQLConfig {
        column_mappings: mappings,
        joins: vec![PostgresJoinConfig {
            table: "orders".to_string(),
            join_table: "users".to_string(),
            on: "users.id = orders.user_id".to_string(),
            r#type: Some("INNER".to_string()),
        }],
        placeholder_start_index: None,
    };

    // Case 1: Only primary table column is used (no joins)
    let result_no_join = compiler
        .compile(&json!({ "name": "Juan" }), Some(config.clone()))
        .unwrap();
    assert_eq!(result_no_join.joins, Vec::<String>::new());

    // Case 2: Joined table is used (join statement must be added)
    let result_with_join = compiler
        .compile(
            &json!({ "name": "Juan", "total": { "$gt": 100 } }),
            Some(config),
        )
        .unwrap();

    let clause = result_with_join.where_clause;
    assert!(
        clause == "users.first_name = $1 AND orders.order_total > $2"
            || clause == "orders.order_total > $1 AND users.first_name = $2"
    );

    if clause.starts_with("users.first_name") {
        assert_eq!(result_with_join.values, vec![json!("Juan"), json!(100)]);
    } else {
        assert_eq!(result_with_join.values, vec![json!(100), json!("Juan")]);
    }
    assert_eq!(
        result_with_join.joins,
        vec!["INNER JOIN orders ON users.id = orders.user_id".to_string()]
    );
}

#[test]
fn test_translate_dot_notation_path() {
    let compiler = PostgreSQLCompiler::new();

    let mut mappings = HashMap::new();
    mappings.insert(
        "imdb".to_string(),
        PostgresColumnConfig {
            table: "movies".to_string(),
            column: "imdb_metadata".to_string(),
        },
    );

    let config = PostgreSQLConfig {
        column_mappings: mappings,
        joins: vec![],
        placeholder_start_index: None,
    };

    let result_dot = compiler
        .compile(&json!({ "imdb.rating": 8 }), Some(config.clone()))
        .unwrap();
    assert_eq!(
        result_dot.where_clause,
        "movies.imdb_metadata->>'rating' = $1"
    );
    assert_eq!(result_dot.values, vec![json!(8)]);

    let result_nested = compiler
        .compile(
            &json!({ "imdb": { "rating": { "$gte": 8.5 } } }),
            Some(config),
        )
        .unwrap();
    assert_eq!(
        result_nested.where_clause,
        "movies.imdb_metadata->>'rating' >= $1"
    );
    assert_eq!(result_nested.values, vec![json!(8.5)]);
}

#[test]
fn test_translate_null_checks() {
    let compiler = PostgreSQLCompiler::new();

    let result_null = compiler
        .compile(&json!({ "deletedAt": null }), None)
        .unwrap();
    assert_eq!(result_null.where_clause, "deletedAt IS NULL");
    assert_eq!(result_null.values, Vec::<serde_json::Value>::new());

    let result_eq_null = compiler
        .compile(&json!({ "deletedAt": { "$eq": null } }), None)
        .unwrap();
    assert_eq!(result_eq_null.where_clause, "deletedAt IS NULL");

    let result_ne_null = compiler
        .compile(&json!({ "deletedAt": { "$ne": null } }), None)
        .unwrap();
    assert_eq!(result_ne_null.where_clause, "deletedAt IS NOT NULL");
}

#[test]
fn test_translate_specialized_operators() {
    let compiler = PostgreSQLCompiler::new();

    let result_begins = compiler
        .compile(&json!({ "email": { "$beginsWith": "admin" } }), None)
        .unwrap();
    assert_eq!(result_begins.where_clause, "email LIKE $1");
    assert_eq!(result_begins.values, vec![json!("admin%")]);

    let result_regex = compiler
        .compile(&json!({ "name": { "$regex": "^J" } }), None)
        .unwrap();
    assert_eq!(result_regex.where_clause, "name ~ $1");
    assert_eq!(result_regex.values, vec![json!("^J")]);

    let result_mod = compiler
        .compile(&json!({ "age": { "$mod": [5, 2] } }), None)
        .unwrap();
    assert_eq!(result_mod.where_clause, "CAST(age AS INTEGER) % $1 = $2");
    assert_eq!(result_mod.values, vec![json!(5), json!(2)]);
}
