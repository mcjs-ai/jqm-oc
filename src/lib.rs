use anyhow::{bail, Result};
use dialoguer::{theme::ColorfulTheme, MultiSelect};
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::time::{Duration, SystemTime};

pub fn strip_jsonc(input: &str) -> String {
    let re = Regex::new(r#"(?s)("(?:\\.|[^"\\])*")|(//[^\n]*|/\*.*?\*/)"#).unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        if let Some(string_match) = caps.get(1) { string_match.as_str().to_string() } else { String::new() }
    }).to_string()
}

pub fn deep_merge(target: &mut Value, source: &Value) {
    if target.is_object() && source.is_object() {
        let target_obj = target.as_object_mut().unwrap();
        let source_obj = source.as_object().unwrap();
        for (k, v) in source_obj {
            if target_obj.contains_key(k) { deep_merge(target_obj.get_mut(k).unwrap(), v); } 
            else { target_obj.insert(k.clone(), v.clone()); }
        }
    } else {
        *target = source.clone();
    }
}

pub fn coerce_and_map(clip: &mut Value, schema_props: &Map<String, Value>, alias_map: &HashMap<String, String>) {
    let obj = match clip.as_object_mut() { Some(o) => o, None => return, };
    let mut new_obj = Map::new();
    
    for (k, v) in obj.iter_mut() {
        let final_key = alias_map.get(k).unwrap_or(k);
        let mut new_v = v.clone();
        
        if let Some(prop_schema) = schema_props.get(final_key) {
            if let Some(schema_type) = prop_schema.get("type").and_then(|t| t.as_str()) {
                if new_v.is_string() {
                    let s = new_v.as_str().unwrap();
                    if schema_type == "boolean" {
                        if s.eq_ignore_ascii_case("true") { new_v = Value::Bool(true); }
                        else if s.eq_ignore_ascii_case("false") { new_v = Value::Bool(false); }
                    } else if schema_type == "number" {
                        if let Ok(n) = s.parse::<f64>() {
                            if let Some(num) = serde_json::Number::from_f64(n) { new_v = Value::Number(num); }
                        }
                    }
                } else if new_v.is_object() && schema_type == "object" {
                    if let Some(nested_props) = prop_schema.get("properties").and_then(|p| p.as_object()) {
                        coerce_and_map(&mut new_v, nested_props, alias_map);
                    }
                }
            }
        }
        new_obj.insert(final_key.to_string(), new_v);
    }
    *clip = Value::Object(new_obj);
}

pub fn fetch_schema(schema_path: Option<&String>) -> Option<Value> {
    if let Some(path_str) = schema_path {
        if path_str.starts_with("http") {
             let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(3)).build().ok()?;
             return client.get(path_str).send().ok().and_then(|r| r.json().ok());
        } else {
             return fs::read_to_string(path_str).ok().and_then(|d| serde_json::from_str(&d).ok());
        }
    }
    let cache_path = dirs::config_dir()?.join("jqm-oc/schema_cache.json");
    let cache_duration = Duration::from_secs(60 * 60 * 24); 

    if cache_path.exists() {
        if let Ok(metadata) = fs::metadata(&cache_path) {
            if let Ok(modified) = metadata.modified() {
                if SystemTime::now().duration_since(modified).unwrap_or_default() < cache_duration {
                    if let Ok(data) = fs::read_to_string(&cache_path) {
                        if let Ok(json) = serde_json::from_str(&data) { return Some(json); }
                    }
                }
            }
        }
    }

    let schema_url = "https://opencode.ai/config.json";
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(3)).build().ok()?;
    if let Ok(response) = client.get(schema_url).send() {
        if let Ok(schema) = response.json::<Value>() {
            let _ = fs::create_dir_all(cache_path.parent()?);
            let _ = fs::write(&cache_path, serde_json::to_string(&schema).unwrap_or_default());
            return Some(schema);
        }
    }
    None
}

pub fn apply_interactive_mode(clip_parsed: &mut Value) -> Result<()> {
    let mut leaves = Vec::new();
    flatten_to_leaves(clip_parsed, "", &mut leaves);
    if leaves.is_empty() { bail!("Clipboard object is empty or invalid for interactive selection."); }

    let display_strings: Vec<String> = leaves.iter().map(|(path, val)| format!("{} = {}", path, val)).collect();
    let defaults: Vec<bool> = vec![true; leaves.len()];

    println!("
[36mInteractive Mode: Select keys to merge.[0m");
    let selection_indices = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Use Space to toggle, Enter to confirm")
        .items(&display_strings)
        .defaults(&defaults)
        .interact()?;

    if selection_indices.is_empty() { bail!("No keys selected."); }

    let selected_leaves: Vec<(String, Value)> = selection_indices.into_iter().map(|i| leaves[i].clone()).collect();
    *clip_parsed = unflatten_leaves(selected_leaves);
    Ok(())
}

fn flatten_to_leaves(val: &Value, prefix: &str, acc: &mut Vec<(String, Value)>) {
    match val {
        Value::Object(map) => {
            for (k, v) in map {
                let new_prefix = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
                flatten_to_leaves(v, &new_prefix, acc);
            }
        }
        _ => acc.push((prefix.to_string(), val.clone())),
    }
}

fn unflatten_leaves(leaves: Vec<(String, Value)>) -> Value {
    let mut root = Map::new();
    for (path, val) in leaves {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = &mut root;
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                current.insert(part.to_string(), val.clone());
            } else {
                if !current.contains_key(*part) || !current.get(*part).unwrap().is_object() {
                    current.insert(part.to_string(), Value::Object(Map::new()));
                }
                current = current.get_mut(*part).unwrap().as_object_mut().unwrap();
            }
        }
    }
    Value::Object(root)
}

pub fn print_changes(path: &str, old: &Value, new: &Value, changes_found: &mut bool) {
    match (old, new) {
        (Value::Object(o1), Value::Object(o2)) => {
            for (k, v2) in o2 {
                let new_path = if path.is_empty() { k.clone() } else { format!("{}.{}", path, k) };
                if let Some(v1) = o1.get(k) {
                    if v1 != v2 { print_changes(&new_path, v1, v2, changes_found); }
                } else {
                    *changes_found = true;
                    println!("  [32m+[0m {} = {}", new_path, v2);
                }
            }
        },
        (Value::Array(a1), Value::Array(a2)) => {
            if a1 != a2 {
                *changes_found = true;
                println!("  [33m~[0m {} (Array updated: {} -> {} items)", path, a1.len(), a2.len());
            }
        },
        (v1, v2) => {
            if v1 != v2 {
                *changes_found = true;
                println!("  [33m~[0m {} : {} -> {}", path, v1, v2);
            }
        }
    }
}
