# Mango Query (Rust)

A Rust implementation of the CouchDB Mango Query specification using the Builder pattern and Serde.

This library serves as:
- A query filter serializer for use in REST and GraphQL APIs.
- A query translation layer between Mango queries and other database schemas (e.g. Mango -> SQL, Mango -> DynamoDB).
- An in-memory query filter for data collections with custom pagination/sorting capabilities.

---

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
mango-query-rs = { version = "0.1.0", features = ["dynamodb"] } # Enable "dynamodb" feature if using DynamoDB compiler
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

---

## Usage Example

### Basic Selector & Options

Using `MangoQueryBuilder` to build CouchDB-compatible Mango query representations.

```rust
use mango_query_rs::MangoQueryBuilder;
use serde_json::json;

fn main() {
    let query = MangoQueryBuilder::new()
        .r#where("status", json!("active"))
        .where_op("age", "$gte", json!(18))
        .where_op("age", "$lte", json!(65))
        .fields(vec!["_id".to_string(), "name".to_string(), "age".to_string()])
        .sort_field("age", Some("desc"))
        .limit(10)
        .skip(0)
        .build();

    println!("{}", serde_json::to_string_pretty(&query).unwrap());
}
```

**Output:**
```json
{
  "selector": {
    "status": "active",
    "age": {
      "$gte": 18,
      "$lte": 65
    }
  },
  "fields": [
    "_id",
    "name",
    "age"
  ],
  "sort": [
    {
      "age": "desc"
    }
  ],
  "limit": 10,
  "skip": 0
}
```

### Nesting & Logical Combinators

```rust
use mango_query_rs::MangoQueryBuilder;
use serde_json::json;

fn main() {
    let query = MangoQueryBuilder::new()
        .r#where("year", json!(1977))
        .or(vec![
            json!({ "director": "George Lucas" }),
            json!({ "director": "Steven Spielberg" }),
        ])
        .build();
}
```

**Output:**
```json
{
  "selector": {
    "year": 1977,
    "$or": [
      {
        "director": "George Lucas"
      },
      {
        "director": "Steven Spielberg"
      }
    ]
  }
}
```

---

## Database Flavours (Query Compilation)

Translate your Mango queries into database-specific filter formats.

### 1. DynamoDB Flavour
> [!NOTE]
> Requires the `dynamodb` feature flag to be enabled.

Compiles queries to AWS SDK-compliant expressions. Output fields (`filter_expression`, `expression_attribute_names`, and `expression_attribute_values`) are wrapped in `Option`, returning `None` if they are empty. It also includes the `key_condition` string field, `index_name: Option<String>` (representing the index resolved from the query's `use_index` property), `exclusive_start_key: Option<HashMap<String, AttributeValue>>` (resolved and parsed from the query's base64-encoded `bookmark` property), and `limit: Option<i32>` (representing the query's execution limit).

You can pass an optional `DynamoDBConfig` to the compiler containing:
- `key_condition`: An initial key condition string.
- `attribute_names`: Initial attribute name placeholders.
- `attribute_values`: Initial attribute value placeholders as a `HashMap<String, AttributeValue>`.

The compiler will automatically merge the configuration's names and values with the placeholders generated during query compilation.

```rust
use aws_sdk_dynamodb::types::AttributeValue;
use mango_query_rs::{DynamoDBCompiler, DynamoDBConfig, FlavourCompiler, MangoQueryBuilder};
use serde_json::json;
use std::collections::HashMap;

fn main() {
    let query = MangoQueryBuilder::new()
        .r#where("status", json!("active"))
        .build();

    let mut attribute_names = HashMap::new();
    attribute_names.insert("#initial_name".to_string(), "initial_val".to_string());

    let mut attribute_values = HashMap::new();
    attribute_values.insert(
        ":initial_value".to_string(),
        AttributeValue::S("initial".to_string()),
    );

    let config = DynamoDBConfig {
        key_condition: "pk = :pk_val".to_string(),
        attribute_names,
        attribute_values,
    };

    let compiler = DynamoDBCompiler::new();
    let query_val = serde_json::to_value(&query).unwrap();
    let result = compiler.compile(&query_val, Some(config)).unwrap();

    println!("Key Condition: {}", result.key_condition);
    // Key Condition: pk = :pk_val
    println!("Filter: {}", result.filter_expression.unwrap());
    // Filter: #attr_status = :val_0
}
```

### 2. PostgreSQL Flavour
Compiles queries to parameterized `where_clause` strings, parameter values, and dynamic `JOIN` statements. Dot notation paths translate to JSONB query extractors (e.g., `column->>'field'`) automatically.

```rust
use mango_query_rs::{
    FlavourCompiler, PostgreSQLCompiler, PostgreSQLConfig,
    PostgresColumnConfig, PostgresJoinConfig, MangoQueryBuilder
};
use serde_json::json;
use std::collections::HashMap;

fn main() {
    let query = MangoQueryBuilder::new()
        .r#where("name", json!("Juan"))
        .where_op("orderTotal", "$gt", json!(100))
        .build();

    let mut column_mappings = HashMap::new();
    column_mappings.insert("name".to_string(), PostgresColumnConfig {
        table: "users".to_string(),
        column: "first_name".to_string(),
    });
    column_mappings.insert("orderTotal".to_string(), PostgresColumnConfig {
        table: "orders".to_string(),
        column: "total".to_string(),
    });

    let config = PostgreSQLConfig {
        column_mappings,
        joins: vec![
            PostgresJoinConfig {
                table: "orders".to_string(),
                join_table: "users".to_string(),
                on: "users.id = orders.user_id".to_string(),
                r#type: Some("INNER".to_string()),
            }
        ],
        placeholder_start_index: None,
    };

    let compiler = PostgreSQLCompiler::new();
    let query_val = serde_json::to_value(&query).unwrap();
    let result = compiler.compile(&query_val, Some(config)).unwrap();

    println!("Where: {}", result.where_clause);
    // Where: users.first_name = $1 AND orders.total > $2
    println!("Joins: {:?}", result.joins);
    // Joins: ["INNER JOIN orders ON users.id = orders.user_id"]
}
```

---

## In-Memory Collection Filtering

Filter, sort, and project local arrays of items using a strictly typed `MangoQuery` with pagination bookmarks and limit boundaries.

```rust
use mango_query_rs::{InMemoryFilter, InMemoryFilterOptions, MangoQueryBuilder};
use serde_json::json;

fn main() {
    let data = vec![
        json!({ "id": "1", "name": "Juan", "status": "active", "age": 30 }),
        json!({ "id": "2", "name": "Pedro", "status": "pending", "age": 40 }),
        json!({ "id": "3", "name": "Diego", "status": "active", "age": 25 }),
        json!({ "id": "4", "name": "Lucas", "status": "active", "age": 35 }),
    ];

    let query = MangoQueryBuilder::new()
        .r#where("status", json!("active"))
        .sort_field("age", Some("asc"))
        .fields(vec!["id".to_string(), "name".to_string(), "age".to_string()])
        .build();

    let pk = vec!["id".to_string()];

    // Page 1: Filter active users with a limit of 2, sorted by age ascending
    let options1 = InMemoryFilterOptions {
        data: &data,
        query: &query,
        pk: &pk,
        limit: Some(2),
        bookmark: None,
    };
    let page1 = InMemoryFilter::filter(options1).unwrap();

    for doc in &page1.docs {
        println!("{}", doc);
    }
    // Prints:
    // {"age":25,"id":"3","name":"Diego"}
    // {"age":30,"id":"1","name":"Juan"}

    println!("Bookmark 1: {:?}", page1.bookmark); // Base64 encoded token
}
```

---

## API Reference

### MangoQueryBuilder Selector Options
- `.r#where(field, value)`: Add field condition (e.g. implicit equality `{ name: "Juan" }`).
- `.where_op(field, operator, value)`: Add field condition operator (e.g. `.where_op("age", "$gte", json!(18))`).
- `.and(selectors)`: Combine selectors using logical `$and`.
- `.or(selectors)`: Combine selectors using logical `$or`.
- `.nor(selectors)`: Combine selectors using logical `$nor`.
- `.not(selector)`: Invert condition using logical `$not`.
- `.text(query)`: Add full text search index expression.

### MangoQueryBuilder Parameters
- `.fields(fields_vec)`: Fields to include in final projected documents.
- `.sort(sort_rules_vec)`: Specify a full sort structure.
- `.sort_field(field, direction)`: Add a single field to sort by fluently (e.g., direction is `Some("desc")` or `None`).
- `.limit(limit_val)`: Limit the result set size.
- `.skip(skip_val)`: Skip the first N documents.
- `.use_index(use_index_val)`: Hint CouchDB to use a specific index.
- `.allow_fallback(boolean)`: Controls index selection fallback.
- `.conflicts(boolean)`: Include document conflict information.
- `.r(quorum)`: Set replica read quorum.
- `.bookmark(string)`: Pagination resume token.
- `.update(boolean)`: Force updating index before execution.
- `.stable(boolean)`: Return results from a stable set of shards.
- `.execution_stats(boolean)`: Include execution statistics in response metadata.

---

## Supported Operators

| Operator | Description | Supported Types |
| --- | --- | --- |
| `$eq` | Equal to | String, Number, Boolean, Null |
| `$ne` | Not equal to | String, Number, Boolean, Null |
| `$gt` | Greater than | Number, String |
| `$gte` | Greater than or equal to | Number, String |
| `$lt` | Less than | Number, String |
| `$lte` | Less than or equal to | Number, String |
| `$in` | Exists in array | Array |
| `$nin` | Does not exist in array | Array |
| `$exists` | Check field existence | Boolean |
| `$type` | Check field JSON type | String (`"null"`, `"boolean"`, `"number"`, `"string"`, `"array"`, `"object"`) |
| `$size` | Array size matching | Integer |
| `$mod` | Divisor & remainder matching | `[Divisor, Remainder]` |
| `$regex` | Regex matching (via regex crate) | String |
| `$beginsWith` | Prefix case-sensitive matching | String |
