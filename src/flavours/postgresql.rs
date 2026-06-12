use crate::flavours::types::FlavourCompiler;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresColumnConfig {
    pub table: String,
    pub column: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresJoinConfig {
    pub table: String,
    pub join_table: String,
    pub on: String,
    pub r#type: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PostgreSQLConfig {
    pub column_mappings: HashMap<String, PostgresColumnConfig>,
    pub joins: Vec<PostgresJoinConfig>,
    pub placeholder_start_index: Option<usize>,
}

pub struct PostgreSQLFilterOutput {
    pub where_clause: String,
    pub values: Vec<Value>,
    pub joins: Vec<String>,
}

pub struct PostgreSQLCompiler;

impl Default for PostgreSQLCompiler {
    fn default() -> Self {
        Self::new()
    }
}

struct PostgresCompilationContext {
    column_mappings: HashMap<String, PostgresColumnConfig>,
    referenced_tables: HashSet<String>,
    values: Vec<Value>,
    placeholder_index: usize,
}

fn is_operator_object(val: &Value) -> bool {
    if let Value::Object(map) = val {
        !map.is_empty() && map.keys().all(|k| k.starts_with('$'))
    } else {
        false
    }
}

fn resolve_sql_field(field_path: &str, context: &mut PostgresCompilationContext) -> String {
    let parts: Vec<&str> = field_path.split('.').collect();
    if parts.is_empty() {
        return field_path.to_string();
    }
    let first_part = parts[0];

    if let Some(mapping) = context.column_mappings.get(first_part) {
        context.referenced_tables.insert(mapping.table.clone());

        if parts.len() > 1 {
            let mut path_expr = format!("{}.{}", mapping.table, mapping.column);
            let json_parts = &parts[1..];
            for (i, p) in json_parts.iter().enumerate() {
                let is_last = i == json_parts.len() - 1;
                let op = if is_last { "->>" } else { "->" };
                path_expr = format!("{}{}'{}'", path_expr, op, p);
            }
            path_expr
        } else {
            format!("{}.{}", mapping.table, mapping.column)
        }
    } else {
        field_path.to_string()
    }
}

fn get_sql_placeholder(val: Value, context: &mut PostgresCompilationContext) -> String {
    context.values.push(val);
    let index = context.placeholder_index;
    context.placeholder_index += 1;
    format!("${}", index)
}

impl PostgreSQLCompiler {
    pub fn new() -> Self {
        Self
    }

    fn compile_selector(
        &self,
        selector: &Value,
        context: &mut PostgresCompilationContext,
    ) -> Result<String, String> {
        let map = match selector {
            Value::Object(m) => m,
            _ => return Ok("".to_string()),
        };

        if map.is_empty() {
            return Ok("".to_string());
        }

        let mut expressions = Vec::new();

        for (key, value) in map {
            if key == "$and" {
                if let Value::Array(arr) = value {
                    let mut sub_exprs = Vec::new();
                    for sub in arr {
                        let expr = self.compile_selector(sub, context)?;
                        if !expr.is_empty() {
                            sub_exprs.push(expr);
                        }
                    }
                    if !sub_exprs.is_empty() {
                        expressions.push(format!("({})", sub_exprs.join(" AND ")));
                    }
                }
            } else if key == "$or" {
                if let Value::Array(arr) = value {
                    let mut sub_exprs = Vec::new();
                    for sub in arr {
                        let expr = self.compile_selector(sub, context)?;
                        if !expr.is_empty() {
                            sub_exprs.push(expr);
                        }
                    }
                    if !sub_exprs.is_empty() {
                        expressions.push(format!("({})", sub_exprs.join(" OR ")));
                    }
                }
            } else if key == "$nor" {
                if let Value::Array(arr) = value {
                    let mut sub_exprs = Vec::new();
                    for sub in arr {
                        let expr = self.compile_selector(sub, context)?;
                        if !expr.is_empty() {
                            sub_exprs.push(expr);
                        }
                    }
                    if !sub_exprs.is_empty() {
                        expressions.push(format!("(NOT ({}))", sub_exprs.join(" OR ")));
                    }
                }
            } else if key == "$not" {
                let expr = self.compile_selector(value, context)?;
                if !expr.is_empty() {
                    expressions.push(format!("(NOT ({}))", expr));
                }
            } else if key.starts_with('$') {
                return Err(format!(
                    "PostgreSQL compiler: Operator '{}' is not supported at the root of a selector.",
                    key
                ));
            } else {
                expressions.push(self.compile_field_condition(key, value, context)?);
            }
        }

        Ok(expressions.join(" AND "))
    }

    fn compile_field_condition(
        &self,
        field: &str,
        value: &Value,
        context: &mut PostgresCompilationContext,
    ) -> Result<String, String> {
        if value.is_object() && !is_operator_object(value) {
            let map = value.as_object().unwrap();
            let mut expressions = Vec::new();
            for (key, val) in map {
                let nested_field = format!("{}.{}", field, key);
                expressions.push(self.compile_field_condition(&nested_field, val, context)?);
            }
            return Ok(expressions.join(" AND "));
        }

        let column_expr = resolve_sql_field(field, context);

        if value.is_null() {
            return Ok(format!("{} IS NULL", column_expr));
        }

        if !value.is_object() || value.is_array() {
            let placeholder = get_sql_placeholder(value.clone(), context);
            return Ok(format!("{} = {}", column_expr, placeholder));
        }

        let map = value.as_object().unwrap();
        let mut op_expressions = Vec::new();

        for (op_key, op_value) in map {
            match op_key.as_str() {
                "$eq" => {
                    if op_value.is_null() {
                        op_expressions.push(format!("{} IS NULL", column_expr));
                    } else {
                        let placeholder = get_sql_placeholder(op_value.clone(), context);
                        op_expressions.push(format!("{} = {}", column_expr, placeholder));
                    }
                }
                "$ne" => {
                    if op_value.is_null() {
                        op_expressions.push(format!("{} IS NOT NULL", column_expr));
                    } else {
                        let placeholder = get_sql_placeholder(op_value.clone(), context);
                        op_expressions.push(format!("{} <> {}", column_expr, placeholder));
                    }
                }
                "$gt" => {
                    let placeholder = get_sql_placeholder(op_value.clone(), context);
                    op_expressions.push(format!("{} > {}", column_expr, placeholder));
                }
                "$gte" => {
                    let placeholder = get_sql_placeholder(op_value.clone(), context);
                    op_expressions.push(format!("{} >= {}", column_expr, placeholder));
                }
                "$lt" => {
                    let placeholder = get_sql_placeholder(op_value.clone(), context);
                    op_expressions.push(format!("{} < {}", column_expr, placeholder));
                }
                "$lte" => {
                    let placeholder = get_sql_placeholder(op_value.clone(), context);
                    op_expressions.push(format!("{} <= {}", column_expr, placeholder));
                }
                "$exists" => {
                    if let Value::Bool(b) = op_value {
                        if *b {
                            op_expressions.push(format!("{} IS NOT NULL", column_expr));
                        } else {
                            op_expressions.push(format!("{} IS NULL", column_expr));
                        }
                    } else {
                        return Err(
                            "PostgreSQL compiler: $exists operator requires a boolean value."
                                .to_string(),
                        );
                    }
                }
                "$beginsWith" => {
                    if let Value::String(s) = op_value {
                        let like_value = format!("{}%", s);
                        let placeholder = get_sql_placeholder(Value::String(like_value), context);
                        op_expressions.push(format!("{} LIKE {}", column_expr, placeholder));
                    } else {
                        return Err(
                            "PostgreSQL compiler: $beginsWith operator requires a string value."
                                .to_string(),
                        );
                    }
                }
                "$regex" => {
                    let placeholder = get_sql_placeholder(op_value.clone(), context);
                    op_expressions.push(format!("{} ~ {}", column_expr, placeholder));
                }
                "$in" => {
                    if let Value::Array(arr) = op_value {
                        if arr.is_empty() {
                            op_expressions.push("FALSE".to_string());
                        } else {
                            let placeholders: Vec<String> = arr
                                .iter()
                                .map(|v| get_sql_placeholder(v.clone(), context))
                                .collect();
                            op_expressions.push(format!(
                                "{} IN ({})",
                                column_expr,
                                placeholders.join(", ")
                            ));
                        }
                    } else {
                        return Err("PostgreSQL compiler: $in operator requires an array value."
                            .to_string());
                    }
                }
                "$nin" => {
                    if let Value::Array(arr) = op_value {
                        if arr.is_empty() {
                            op_expressions.push("TRUE".to_string());
                        } else {
                            let placeholders: Vec<String> = arr
                                .iter()
                                .map(|v| get_sql_placeholder(v.clone(), context))
                                .collect();
                            op_expressions.push(format!(
                                "{} NOT IN ({})",
                                column_expr,
                                placeholders.join(", ")
                            ));
                        }
                    } else {
                        return Err(
                            "PostgreSQL compiler: $nin operator requires an array value."
                                .to_string(),
                        );
                    }
                }
                "$mod" => {
                    if let Value::Array(arr) = op_value {
                        if arr.len() == 2 {
                            let div_placeholder = get_sql_placeholder(arr[0].clone(), context);
                            let rem_placeholder = get_sql_placeholder(arr[1].clone(), context);
                            op_expressions.push(format!(
                                "CAST({} AS INTEGER) % {} = {}",
                                column_expr, div_placeholder, rem_placeholder
                            ));
                        } else {
                            return Err("PostgreSQL compiler: $mod operator requires an array of [divisor, remainder].".to_string());
                        }
                    } else {
                        return Err(
                            "PostgreSQL compiler: $mod operator requires an array value."
                                .to_string(),
                        );
                    }
                }
                _ => {
                    return Err(format!(
                        "PostgreSQL compiler: Operator '{}' is not supported for field conditions.",
                        op_key
                    ));
                }
            }
        }

        Ok(op_expressions.join(" AND "))
    }
}

impl FlavourCompiler<PostgreSQLFilterOutput, PostgreSQLConfig> for PostgreSQLCompiler {
    fn compile(
        &self,
        query: &Value,
        config: Option<PostgreSQLConfig>,
    ) -> Result<PostgreSQLFilterOutput, String> {
        let selector = if let Some(sel) = query.get("selector") {
            sel
        } else {
            query
        };

        let conf = config.unwrap_or_default();

        let mut context = PostgresCompilationContext {
            column_mappings: conf.column_mappings,
            referenced_tables: HashSet::new(),
            values: Vec::new(),
            placeholder_index: conf.placeholder_start_index.unwrap_or(1),
        };

        let where_clause = self.compile_selector(selector, &mut context)?;

        let mut join_strings = Vec::new();
        for join in conf.joins {
            if context.referenced_tables.contains(&join.table) {
                let join_type = join.r#type.unwrap_or_else(|| "LEFT".to_string());
                join_strings.push(format!("{} JOIN {} ON {}", join_type, join.table, join.on));
            }
        }

        Ok(PostgreSQLFilterOutput {
            where_clause,
            values: context.values,
            joins: join_strings,
        })
    }
}
