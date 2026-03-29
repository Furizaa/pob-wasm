use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct GemData {
    pub id: String,
    pub display_name: String,
    pub is_support: bool,
    pub skill_types: Vec<u32>,
}

pub type GemsMap = HashMap<String, GemData>;
