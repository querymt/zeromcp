use std::collections::HashMap;

use anyhow::Result;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

pub fn hashmap_to_header_map(raw_map: &HashMap<String, String>) -> Result<HeaderMap> {
    let mut hdrs = HeaderMap::new();
    for (name_str, value_str) in raw_map {
        let name = HeaderName::from_bytes(name_str.as_bytes())?;
        let value = HeaderValue::from_str(&value_str)?;
        hdrs.insert(name, value);
    }
    Ok(hdrs)
}
