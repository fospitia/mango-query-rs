use crate::models::{MangoQuery, SortRule, UseIndex};
use serde_json::{Map, Value};
use std::collections::HashMap;

pub struct MangoQueryBuilder {
    query: MangoQuery,
}

impl Default for MangoQueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MangoQueryBuilder {
    pub fn new() -> Self {
        Self {
            query: MangoQuery {
                selector: Value::Object(Map::new()),
                ..Default::default()
            },
        }
    }

    /// Add a condition to the selector.
    /// Supports implicit equality (e.g. where("name", Value::String("Juan")))
    /// or explicit operator objects (e.g. where("age", json!({ "$gte": 18 })))
    pub fn r#where(mut self, field: &str, val: Value) -> Self {
        if let Value::Object(ref mut map) = self.query.selector {
            map.insert(field.to_string(), val);
        }
        self
    }

    /// Add a field condition operator fluently (e.g. where_op("age", "$gte", json!(18)))
    /// Merges operators for the same field automatically.
    pub fn where_op(mut self, field: &str, op: &str, val: Value) -> Self {
        if let Value::Object(ref mut map) = self.query.selector {
            let entry = map
                .entry(field.to_string())
                .or_insert_with(|| Value::Object(Map::new()));

            // If the existing entry is an object and not an array/primitive, we merge the operator
            if let Value::Object(op_map) = entry {
                op_map.insert(op.to_string(), val);
            } else {
                // Otherwise overwrite (or if it was a primitive, convert it to operator object)
                let mut new_op_map = Map::new();
                new_op_map.insert(op.to_string(), val);
                *entry = Value::Object(new_op_map);
            }
        }
        self
    }

    /// Logical AND: Matches if all selectors match.
    pub fn and(mut self, selectors: Vec<Value>) -> Self {
        if let Value::Object(ref mut map) = self.query.selector {
            map.insert("$and".to_string(), Value::Array(selectors));
        }
        self
    }

    /// Logical OR: Matches if any of the selectors match.
    pub fn or(mut self, selectors: Vec<Value>) -> Self {
        if let Value::Object(ref mut map) = self.query.selector {
            map.insert("$or".to_string(), Value::Array(selectors));
        }
        self
    }

    /// Logical NOR: Matches if none of the selectors match.
    pub fn nor(mut self, selectors: Vec<Value>) -> Self {
        if let Value::Object(ref mut map) = self.query.selector {
            map.insert("$nor".to_string(), Value::Array(selectors));
        }
        self
    }

    /// Logical NOT: Matches if the given selector does not match.
    pub fn not(mut self, selector: Value) -> Self {
        if let Value::Object(ref mut map) = self.query.selector {
            map.insert("$not".to_string(), selector);
        }
        self
    }

    /// Perform a text search using a search or nouveau index.
    pub fn text(mut self, query_str: &str) -> Self {
        if let Value::Object(ref mut map) = self.query.selector {
            map.insert("$text".to_string(), Value::String(query_str.to_string()));
        }
        self
    }

    /// Restrict output fields to this list.
    pub fn fields(mut self, fields: Vec<String>) -> Self {
        self.query.fields = Some(fields);
        self
    }

    /// Specify a full sort structure.
    pub fn sort(mut self, sort_rules: Vec<SortRule>) -> Self {
        self.query.sort = Some(sort_rules);
        self
    }

    /// Add a single field to sort by fluently (can be chained).
    pub fn sort_field(mut self, field: &str, direction: Option<&str>) -> Self {
        let dir = direction.unwrap_or("asc").to_string();
        let rule = if dir == "asc" {
            SortRule::Field(field.to_string())
        } else {
            let mut map = HashMap::new();
            map.insert(field.to_string(), dir);
            SortRule::FieldDirection(map)
        };

        if let Some(ref mut existing_sort) = self.query.sort {
            existing_sort.push(rule);
        } else {
            self.query.sort = Some(vec![rule]);
        }
        self
    }

    /// Limit number of documents returned.
    pub fn limit(mut self, limit: usize) -> Self {
        self.query.limit = Some(limit);
        self
    }

    /// Skip the first N documents.
    pub fn skip(mut self, skip: usize) -> Self {
        self.query.skip = Some(skip);
        self
    }

    /// Force using a specific index.
    pub fn use_index(mut self, index: UseIndex) -> Self {
        self.query.use_index = Some(index);
        self
    }

    /// Controls index selection fallback.
    pub fn allow_fallback(mut self, allow: bool) -> Self {
        self.query.allow_fallback = Some(allow);
        self
    }

    /// Include document conflict information.
    pub fn conflicts(mut self, conflicts: bool) -> Self {
        self.query.conflicts = Some(conflicts);
        self
    }

    /// Set replica read quorum.
    pub fn r(mut self, quorum: usize) -> Self {
        self.query.r = Some(quorum);
        self
    }

    /// CouchDB bookmark for pagination.
    pub fn bookmark(mut self, bookmark_str: &str) -> Self {
        self.query.bookmark = Some(bookmark_str.to_string());
        self
    }

    /// Force updating index before execution.
    pub fn update(mut self, update: bool) -> Self {
        self.query.update = Some(update);
        self
    }

    /// Return results from a stable set of shards.
    pub fn stable(mut self, stable: bool) -> Self {
        self.query.stable = Some(stable);
        self
    }

    /// Return execution stats along with the query result.
    pub fn execution_stats(mut self, stats: bool) -> Self {
        self.query.execution_stats = Some(stats);
        self
    }

    /// Builds and returns the MangoQuery.
    pub fn build(self) -> MangoQuery {
        self.query
    }
}
