use crate::models::{MangoQuery, SortRule};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashMap;

pub struct InMemoryFilterOptions<'a> {
    pub data: &'a [Value],
    pub query: &'a MangoQuery,
    pub pk: &'a [String],
    pub limit: Option<usize>,
    pub bookmark: Option<&'a str>,
}

pub struct InMemoryFilterResult {
    pub docs: Vec<Value>,
    pub bookmark: Option<String>,
}

fn get_value_by_path(obj: &Value, path: &str) -> Option<Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = obj;
    for part in parts {
        match current {
            Value::Object(map) => {
                if let Some(next) = map.get(part) {
                    current = next;
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(current.clone())
}

fn set_value_by_path(mut obj: &mut Value, path: &str, value: Value) {
    let parts: Vec<&str> = path.split('.').collect();
    for (i, &part) in parts.iter().enumerate() {
        if !obj.is_object() {
            *obj = Value::Object(Map::new());
        }
        let map = obj.as_object_mut().unwrap();
        if i == parts.len() - 1 {
            map.insert(part.to_string(), value);
            return;
        } else {
            if !map.contains_key(part) || !map.get(part).unwrap().is_object() {
                map.insert(part.to_string(), Value::Object(Map::new()));
            }
            obj = map.get_mut(part).unwrap();
        }
    }
}

fn is_operator_object(val: &Value) -> bool {
    if let Value::Object(map) = val {
        !map.is_empty() && map.keys().all(|k| k.starts_with('$'))
    } else {
        false
    }
}

fn eval_operator(field_val: &Value, op: &str, op_val: &Value) -> bool {
    match op {
        "$eq" => field_val == op_val,
        "$ne" => field_val != op_val,
        "$gt" => match (field_val, op_val) {
            (Value::Number(a), Value::Number(b)) => a.as_f64() > b.as_f64(),
            (Value::String(a), Value::String(b)) => a > b,
            _ => false,
        },
        "$gte" => match (field_val, op_val) {
            (Value::Number(a), Value::Number(b)) => a.as_f64() >= b.as_f64(),
            (Value::String(a), Value::String(b)) => a >= b,
            _ => false,
        },
        "$lt" => match (field_val, op_val) {
            (Value::Number(a), Value::Number(b)) => a.as_f64() < b.as_f64(),
            (Value::String(a), Value::String(b)) => a < b,
            _ => false,
        },
        "$lte" => match (field_val, op_val) {
            (Value::Number(a), Value::Number(b)) => a.as_f64() <= b.as_f64(),
            (Value::String(a), Value::String(b)) => a <= b,
            _ => false,
        },
        "$exists" => {
            let exists = !field_val.is_null();
            if let Value::Bool(b) = op_val {
                exists == *b
            } else {
                false
            }
        }
        "$type" => {
            if let Value::String(t) = op_val {
                match t.as_str() {
                    "null" => field_val.is_null(),
                    "boolean" => field_val.is_boolean(),
                    "number" => field_val.is_number(),
                    "string" => field_val.is_string(),
                    "array" => field_val.is_array(),
                    "object" => field_val.is_object(),
                    _ => false,
                }
            } else {
                false
            }
        }
        "$in" => {
            if let Value::Array(arr) = op_val {
                if let Value::Array(field_arr) = field_val {
                    field_arr.iter().any(|fv| arr.contains(fv))
                } else {
                    arr.contains(field_val)
                }
            } else {
                false
            }
        }
        "$nin" => {
            if let Value::Array(arr) = op_val {
                if let Value::Array(field_arr) = field_val {
                    !field_arr.iter().any(|fv| arr.contains(fv))
                } else {
                    !arr.contains(field_val)
                }
            } else {
                false
            }
        }
        "$size" => {
            if let Value::Array(arr) = field_val {
                if let Value::Number(num) = op_val {
                    arr.len() as u64 == num.as_u64().unwrap_or(0)
                } else {
                    false
                }
            } else {
                false
            }
        }
        "$mod" => {
            if let (Value::Number(val_num), Value::Array(arr)) = (field_val, op_val) {
                if arr.len() == 2 {
                    let divisor = arr[0].as_i64();
                    let remainder = arr[1].as_i64();
                    let val = val_num.as_i64();
                    matches!((divisor, remainder, val), (Some(d), Some(r), Some(v)) if d != 0 && v % d == r)
                } else {
                    false
                }
            } else {
                false
            }
        }
        "$regex" => {
            if let (Value::String(val_str), Value::String(pattern)) = (field_val, op_val) {
                if let Ok(re) = Regex::new(pattern) {
                    re.is_match(val_str)
                } else {
                    false
                }
            } else {
                false
            }
        }
        "$beginsWith" => {
            if let (Value::String(val_str), Value::String(prefix)) = (field_val, op_val) {
                val_str.starts_with(prefix)
            } else {
                false
            }
        }
        _ => false,
    }
}

fn matches_field(item: &Value, path: &str, constraint: &Value) -> bool {
    if constraint.is_object() && !is_operator_object(constraint) {
        let map = constraint.as_object().unwrap();
        for (key, val) in map {
            let nested_path = format!("{}.{}", path, key);
            if !matches_field(item, &nested_path, val) {
                return false;
            }
        }
        return true;
    }

    let field_value = get_value_by_path(item, path).unwrap_or(Value::Null);

    if !constraint.is_object() || constraint.is_array() {
        return field_value == *constraint;
    }

    let map = constraint.as_object().unwrap();
    for (op_key, op_value) in map {
        if !eval_operator(&field_value, op_key, op_value) {
            return false;
        }
    }
    true
}

fn matches_selector(item: &Value, selector: &Value) -> bool {
    let map = match selector {
        Value::Object(m) => m,
        _ => return false,
    };

    for (key, value) in map {
        if key == "$and" {
            if let Value::Array(arr) = value {
                if !arr.iter().all(|sub| matches_selector(item, sub)) {
                    return false;
                }
            } else {
                return false;
            }
        } else if key == "$or" {
            if let Value::Array(arr) = value {
                if !arr.iter().any(|sub| matches_selector(item, sub)) {
                    return false;
                }
            } else {
                return false;
            }
        } else if key == "$nor" {
            if let Value::Array(arr) = value {
                if arr.iter().any(|sub| matches_selector(item, sub)) {
                    return false;
                }
            } else {
                return false;
            }
        } else if key == "$not" {
            if matches_selector(item, value) {
                return false;
            }
        } else if key.starts_with('$') {
            return false;
        } else {
            if !matches_field(item, key, value) {
                return false;
            }
        }
    }
    true
}

fn matches_pk(item: &Value, bookmark_pk: &HashMap<String, Value>, pk_config: &[String]) -> bool {
    pk_config.iter().all(|field| {
        let val = get_value_by_path(item, field).unwrap_or(Value::Null);
        if let Some(expected) = bookmark_pk.get(field) {
            val == *expected
        } else {
            false
        }
    })
}

fn get_pk_value(item: &Value, pk_config: &[String]) -> HashMap<String, Value> {
    let mut result = HashMap::new();
    for field in pk_config {
        let val = get_value_by_path(item, field).unwrap_or(Value::Null);
        result.insert(field.to_string(), val);
    }
    result
}

fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    let type_ord = |val: &Value| -> u8 {
        match val {
            Value::Null => 0,
            Value::Bool(_) => 1,
            Value::Number(_) => 2,
            Value::String(_) => 3,
            Value::Array(_) => 4,
            Value::Object(_) => 5,
        }
    };

    let ord_a = type_ord(a);
    let ord_b = type_ord(b);

    if ord_a != ord_b {
        return ord_a.cmp(&ord_b);
    }

    match (a, b) {
        (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (Value::Number(x), Value::Number(y)) => {
            let f_x = x.as_f64().unwrap_or(0.0);
            let f_y = y.as_f64().unwrap_or(0.0);
            f_x.partial_cmp(&f_y).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Value::String(x), Value::String(y)) => x.cmp(y),
        (Value::Array(x), Value::Array(y)) => {
            for (el_x, el_y) in x.iter().zip(y.iter()) {
                let ord = compare_values(el_x, el_y);
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
            x.len().cmp(&y.len())
        }
        (Value::Object(x), Value::Object(y)) => {
            if x.len() != y.len() {
                x.len().cmp(&y.len())
            } else {
                for (k_x, k_y) in x.keys().zip(y.keys()) {
                    let ord = k_x.cmp(k_y);
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
                for (v_x, v_y) in x.values().zip(y.values()) {
                    let ord = compare_values(v_x, v_y);
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
                std::cmp::Ordering::Equal
            }
        }
        _ => std::cmp::Ordering::Equal,
    }
}

fn sort_items(items: &mut [Value], sort_rules: &[SortRule]) {
    items.sort_by(|a, b| {
        for rule in sort_rules {
            let (field, direction) = match rule {
                SortRule::Field(f) => (f.as_str(), "asc"),
                SortRule::FieldDirection(map) => {
                    if let Some((f, dir)) = map.iter().next() {
                        (f.as_str(), dir.as_str())
                    } else {
                        continue;
                    }
                }
            };

            let val_a = get_value_by_path(a, field);
            let val_b = get_value_by_path(b, field);

            let ordering = match (val_a, val_b) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
                (Some(a_val), Some(b_val)) => compare_values(&a_val, &b_val),
            };

            if ordering != std::cmp::Ordering::Equal {
                return if direction == "desc" {
                    ordering.reverse()
                } else {
                    ordering
                };
            }
        }
        std::cmp::Ordering::Equal
    });
}

fn project_fields(item: &Value, fields: &[String]) -> Value {
    let mut projected = Value::Object(Map::new());
    for field in fields {
        if let Some(val) = get_value_by_path(item, field) {
            set_value_by_path(&mut projected, field, val);
        }
    }
    projected
}

pub struct InMemoryFilter;

impl InMemoryFilter {
    /// Evaluates if a single item matches the given Mango Selector.
    pub fn matches(item: &Value, query: &Value) -> bool {
        let selector = if let Some(sel) = query.get("selector") {
            sel
        } else {
            query
        };
        matches_selector(item, selector)
    }

    /// Filters a vector of items with a Mango Query, returning the sliced items
    /// and a pagination bookmark if the limit is reached before ending evaluation.
    /// Applies sorting and field projection from the query settings.
    pub fn filter(options: InMemoryFilterOptions) -> Result<InMemoryFilterResult, String> {
        let InMemoryFilterOptions {
            data,
            query,
            pk,
            limit,
            bookmark,
        } = options;

        let limit_val = limit.unwrap_or(25);
        let has_sort = query.sort.as_ref().map(|s| !s.is_empty()).unwrap_or(false);

        let mut final_docs = Vec::new();
        let mut next_bookmark = None;

        if has_sort {
            // 1. Filter the entire dataset
            let mut all_matched: Vec<Value> = data
                .iter()
                .filter(|item| matches_selector(item, &query.selector))
                .cloned()
                .collect();

            // 2. Sort the entire matching dataset
            sort_items(&mut all_matched, query.sort.as_ref().unwrap());

            // 3. Find starting index based on bookmark
            let mut start_index = 0;
            if let Some(bmark) = bookmark {
                let decoded_bytes = STANDARD.decode(bmark).map_err(|e| e.to_string())?;
                let decoded_pk: HashMap<String, Value> =
                    serde_json::from_slice(&decoded_bytes).map_err(|e| e.to_string())?;

                if let Some(found_idx) = sorted_matched_index(&all_matched, &decoded_pk, pk) {
                    start_index = found_idx + 1;
                }
            }

            // 4. Slice up to limit
            let end_index = std::cmp::min(start_index + limit_val, all_matched.len());
            final_docs = all_matched[start_index..end_index].to_vec();

            // 5. Generate bookmark
            let reached_limit = final_docs.len() == limit_val;
            let has_more = (start_index + final_docs.len()) < all_matched.len();
            if reached_limit && has_more && !final_docs.is_empty() {
                let last_item = &final_docs[final_docs.len() - 1];
                let pk_val = get_pk_value(last_item, pk);
                let pk_json = serde_json::to_string(&pk_val).map_err(|e| e.to_string())?;
                next_bookmark = Some(STANDARD.encode(pk_json));
            }
        } else {
            // Optimized early break path
            let mut start_index = 0;
            if let Some(bmark) = bookmark {
                let decoded_bytes = STANDARD.decode(bmark).map_err(|e| e.to_string())?;
                let decoded_pk: HashMap<String, Value> =
                    serde_json::from_slice(&decoded_bytes).map_err(|e| e.to_string())?;

                if let Some(found_idx) = sorted_matched_index(data, &decoded_pk, pk) {
                    start_index = found_idx + 1;
                }
            }

            let mut last_evaluated_idx = (start_index as i64) - 1;
            for (i, item) in data.iter().enumerate().skip(start_index) {
                last_evaluated_idx = i as i64;

                if matches_selector(item, &query.selector) {
                    final_docs.push(item.clone());
                }

                if final_docs.len() == limit_val {
                    break;
                }
            }

            let reached_limit = final_docs.len() == limit_val;
            let has_more = last_evaluated_idx < (data.len() as i64) - 1;
            if reached_limit && has_more && last_evaluated_idx >= 0 {
                let last_item = &data[last_evaluated_idx as usize];
                let pk_val = get_pk_value(last_item, pk);
                let pk_json = serde_json::to_string(&pk_val).map_err(|e| e.to_string())?;
                next_bookmark = Some(STANDARD.encode(pk_json));
            }
        }

        // Apply projection
        let projected_docs = if let Some(ref fields_list) = query.fields {
            final_docs
                .iter()
                .map(|doc| project_fields(doc, fields_list))
                .collect()
        } else {
            final_docs
        };

        Ok(InMemoryFilterResult {
            docs: projected_docs,
            bookmark: next_bookmark,
        })
    }
}

fn sorted_matched_index(
    items: &[Value],
    decoded_pk: &HashMap<String, Value>,
    pk_config: &[String],
) -> Option<usize> {
    items
        .iter()
        .position(|item| matches_pk(item, decoded_pk, pk_config))
}
