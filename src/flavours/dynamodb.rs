use crate::flavours::types::FlavourCompiler;
use serde_json::Value;
use std::collections::HashMap;

pub struct DynamoDBFilterOutput {
    pub filter_expression: String,
    pub expression_attribute_names: HashMap<String, String>,
    pub expression_attribute_values: HashMap<String, Value>,
}

pub struct DynamoDBCompiler;

impl Default for DynamoDBCompiler {
    fn default() -> Self {
        Self::new()
    }
}

struct CompilationContext {
    attribute_names: HashMap<String, String>,
    attribute_values: HashMap<String, Value>,
    value_counter: usize,
}

fn is_operator_object(val: &Value) -> bool {
    if let Value::Object(map) = val {
        !map.is_empty() && map.keys().all(|k| k.starts_with('$'))
    } else {
        false
    }
}

fn get_attribute_name_placeholder(path: &str, context: &mut CompilationContext) -> String {
    let parts: Vec<&str> = path.split('.').collect();
    let placeholders: Vec<String> = parts
        .into_iter()
        .map(|part| {
            let clean_part: String = part
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let placeholder = format!("#attr_{}", clean_part);
            context
                .attribute_names
                .insert(placeholder.clone(), part.to_string());
            placeholder
        })
        .collect();
    placeholders.join(".")
}

fn get_value_placeholder(value: Value, context: &mut CompilationContext) -> String {
    let placeholder = format!(":val_{}", context.value_counter);
    context.value_counter += 1;
    context.attribute_values.insert(placeholder.clone(), value);
    placeholder
}

impl DynamoDBCompiler {
    pub fn new() -> Self {
        Self
    }

    fn compile_selector(
        &self,
        selector: &Value,
        context: &mut CompilationContext,
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
                    "DynamoDB compiler: Operator '{}' is not supported at the root of a selector.",
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
        context: &mut CompilationContext,
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

        let attr_path = get_attribute_name_placeholder(field, context);

        if !value.is_object() || value.is_array() {
            let val_placeholder = get_value_placeholder(value.clone(), context);
            return Ok(format!("{} = {}", attr_path, val_placeholder));
        }

        let map = value.as_object().unwrap();
        let mut op_expressions = Vec::new();

        for (op_key, op_value) in map {
            match op_key.as_str() {
                "$eq" => {
                    let val_placeholder = get_value_placeholder(op_value.clone(), context);
                    op_expressions.push(format!("{} = {}", attr_path, val_placeholder));
                }
                "$ne" => {
                    let val_placeholder = get_value_placeholder(op_value.clone(), context);
                    op_expressions.push(format!("{} <> {}", attr_path, val_placeholder));
                }
                "$gt" => {
                    let val_placeholder = get_value_placeholder(op_value.clone(), context);
                    op_expressions.push(format!("{} > {}", attr_path, val_placeholder));
                }
                "$gte" => {
                    let val_placeholder = get_value_placeholder(op_value.clone(), context);
                    op_expressions.push(format!("{} >= {}", attr_path, val_placeholder));
                }
                "$lt" => {
                    let val_placeholder = get_value_placeholder(op_value.clone(), context);
                    op_expressions.push(format!("{} < {}", attr_path, val_placeholder));
                }
                "$lte" => {
                    let val_placeholder = get_value_placeholder(op_value.clone(), context);
                    op_expressions.push(format!("{} <= {}", attr_path, val_placeholder));
                }
                "$exists" => {
                    if let Value::Bool(b) = op_value {
                        if *b {
                            op_expressions.push(format!("attribute_exists({})", attr_path));
                        } else {
                            op_expressions.push(format!("attribute_not_exists({})", attr_path));
                        }
                    } else {
                        return Err(
                            "DynamoDB compiler: $exists operator requires a boolean value."
                                .to_string(),
                        );
                    }
                }
                "$beginsWith" => {
                    let val_placeholder = get_value_placeholder(op_value.clone(), context);
                    op_expressions.push(format!("begins_with({}, {})", attr_path, val_placeholder));
                }
                "$type" => {
                    if let Value::String(t) = op_value {
                        let type_str = match t.as_str() {
                            "string" => "S",
                            "number" => "N",
                            "boolean" => "BOOL",
                            "null" => "NULL",
                            "array" => "L",
                            "object" => "M",
                            _ => {
                                return Err(format!(
                                    "DynamoDB compiler: Unsupported $type value '{}'.",
                                    t
                                ));
                            }
                        };
                        let val_placeholder =
                            get_value_placeholder(Value::String(type_str.to_string()), context);
                        op_expressions.push(format!(
                            "attribute_type({}, {})",
                            attr_path, val_placeholder
                        ));
                    } else {
                        return Err("DynamoDB compiler: $type operator requires a string value."
                            .to_string());
                    }
                }
                "$in" => {
                    if let Value::Array(arr) = op_value {
                        if arr.is_empty() {
                            op_expressions.push(format!("size({}) < 0", attr_path));
                        } else {
                            let placeholders: Vec<String> = arr
                                .iter()
                                .map(|v| get_value_placeholder(v.clone(), context))
                                .collect();
                            op_expressions.push(format!(
                                "{} IN ({})",
                                attr_path,
                                placeholders.join(", ")
                            ));
                        }
                    } else {
                        return Err(
                            "DynamoDB compiler: $in operator requires an array value.".to_string()
                        );
                    }
                }
                "$nin" => {
                    if let Value::Array(arr) = op_value {
                        if arr.is_empty() {
                            op_expressions.push(format!("attribute_exists({})", attr_path));
                        } else {
                            let placeholders: Vec<String> = arr
                                .iter()
                                .map(|v| get_value_placeholder(v.clone(), context))
                                .collect();
                            op_expressions.push(format!(
                                "NOT ({} IN ({}))",
                                attr_path,
                                placeholders.join(", ")
                            ));
                        }
                    } else {
                        return Err(
                            "DynamoDB compiler: $nin operator requires an array value.".to_string()
                        );
                    }
                }
                _ => {
                    return Err(format!(
                        "DynamoDB compiler: Operator '{}' is not supported for field conditions.",
                        op_key
                    ));
                }
            }
        }

        Ok(op_expressions.join(" AND "))
    }
}

impl FlavourCompiler<DynamoDBFilterOutput, ()> for DynamoDBCompiler {
    fn compile(&self, query: &Value, _config: Option<()>) -> Result<DynamoDBFilterOutput, String> {
        let selector = if let Some(sel) = query.get("selector") {
            sel
        } else {
            query
        };

        let mut context = CompilationContext {
            attribute_names: HashMap::new(),
            attribute_values: HashMap::new(),
            value_counter: 0,
        };

        let filter_expression = self.compile_selector(selector, &mut context)?;

        Ok(DynamoDBFilterOutput {
            filter_expression,
            expression_attribute_names: context.attribute_names,
            expression_attribute_values: context.attribute_values,
        })
    }
}
