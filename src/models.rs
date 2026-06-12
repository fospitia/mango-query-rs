use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum SortRule {
    Field(String),
    FieldDirection(HashMap<String, String>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum UseIndex {
    DesignDoc(String),
    DesignDocIndex((String, String)),
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct MangoQuery {
    pub selector: Value,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<Vec<SortRule>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_index: Option<UseIndex>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_fallback: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflicts: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub r: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bookmark: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub update: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stable: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_stats: Option<bool>,
}
