use regex::Regex;
use serde_json::{Map, Value};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, exit};

// Safely strip JSONC comments while preserving valid strings
fn strip_jsonc(input: &str) -> String {
    let re = Regex::new(r#"(?s)("(?:\\.|[^"\\])*")|(//[^\n]*|/\*.*?\*/)"#).unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        if let Some(string_match) = caps.get(1) {
            string_match.as_str().to_string()
        } else {
            String::new()
        }
    })
    .to_string()
}

// Deep merge the source Value into the target Value
fn deep_merge(target: &mut Value, source: &Value) {
    if target.is_object() && source.is_object() {
        let target_obj = target.as_object_mut().unwrap();
        let source_obj = source.as_object().unwrap();

        for (k, v) in source_obj {
            if target_obj.contains_key(k) {
                deep_merge(target_obj.get_mut(k).unwrap(), v);
            } else {
                target_obj.insert(k.clone(), v.clone());
            }
        }
    } else {
        *target = source.clone();
    }
}

// Coerce values and translate custom keys to the official schema
fn coerce_and_map(clip: &mut Value, schema_props: &Map<String, Value>) {
    let obj = match clip.as_object_mut() {
        Some(o) => o,
        None => return,
    };

    let mut new_obj = Map::new();

    for (k, v) in obj.iter_mut() {
        let final_key = match k.as_str() {
            "mcpServers" => "mcp",
            "lspServers" => "lsp",
            "customAgents" => "agent",
            other => other,
        };

        let mut new_v = v.clone();

        if let Some(prop_schema) = schema_props.get(final_key) {
            if let Some(schema_type) = prop_schema.get("type").and_then(|t| t.as_str()) {
                if new_v.is_string() {
                    let s = new_v.as_str().unwrap();
                    if schema_type == "boolean" {
                        if s.eq_ignore_ascii_case("true") {
                            new_v = Value::Bool(true);
                        } else if s.eq_ignore_ascii_case("false") {
                            new_v = Value::Bool(false);
                        }
                    } else if schema_type == "number" {
                        if let Ok(n) = s.parse::<f64>() {
                            if let Some(num) = serde_json::Number::from_f64(n) {
                                new_v = Value::Number(num);
                            }
                        }
                    }
                } else if new_v.is_object() && schema_type == "object" {
                    if let Some(nested_props) = prop_schema.get("properties").and_then(|p| p.as_object()) {
                        coerce_and_map(&mut new_v, nested_props);
                    }
                }
            }
        }
        new_obj.insert(final_key.to_string(), new_v);
    }

    *clip = Value::Object(new_obj);
}

// Recursively print the differences between the old target and the new target
fn print_changes(path: &str, old: &Value, new: &Value, changes_found: &mut bool) {
    match (old, new) {
        (Value::Object(o1), Value::Object(o2)) => {
            for (k, v2) in o2 {
                let new_path = if path.is_empty() { k.clone() } else { format!("{}.{}", path, k) };
                if let Some(v1) = o1.get(k) {
                    if v1 != v2 {
                        print_changes(&new_path, v1, v2, changes_found);
                    }
                } else {
                    *changes_found = true;
                    // Green '+' for new keys
                    println!("  \x1b[32m+\x1b[0m {} = {}", new_path, v2);
                }
            }
        },
        (Value::Array(a1), Value::Array(a2)) => {
            if a1 != a2 {
                *changes_found = true;
                // Yellow '~' for modified arrays
                println!("  \x1b[33m~\x1b[0m {} (Array updated: {} -> {} items)", path, a1.len(), a2.len());
            }
        },
        (v1, v2) => {
            if v1 != v2 {
                *changes_found = true;
                // Yellow '~' for modified primitives
                println!("  \x1b[33m~\x1b[0m {} : {} -> {}", path, v1, v2);
            }
        }
    }
}

fn main() {
    let target_file = env::args().nth(1).unwrap_or_else(|| {
        let home = env::var("HOME").expect("HOME environment variable not set");
        format!("{}/.config/opencode/opencode.jsonc", home)
    });

    let target_path = PathBuf::from(&target_file);

    let clip_output = Command::new("xclip")
        .args(["-selection", "clipboard", "-o"])
        .output()
        .unwrap_or_else(|_| {
            eprintln!("Error: Failed to execute xclip. Is it installed?");
            exit(1);
        });

    if !clip_output.status.success() {
        eprintln!("Error: xclip returned a non-zero status. Is your clipboard empty?");
        exit(1);
    }

    let clip_raw = String::from_utf8_lossy(&clip_output.stdout).to_string();
    if clip_raw.trim().is_empty() {
        eprintln!("Error: Clipboard is empty.");
        exit(1);
    }

    let mut target_data: Value = if target_path.exists() {
        let raw_target = fs::read_to_string(&target_path).unwrap_or_default();
        let clean_target = strip_jsonc(&raw_target);
        serde_json::from_str(&clean_target).unwrap_or_else(|_| Value::Object(Map::new()))
    } else {
        Value::Object(Map::new())
    };

    let clean_clip = strip_jsonc(&clip_raw);
    let mut clip_parsed: Value = serde_json::from_str(&clean_clip).unwrap_or_else(|e| {
        eprintln!("Error: Clipboard data is not valid JSON or JSONC.\n{}", e);
        exit(1);
    });

    let schema_url = "https://opencode.ai/config.json";
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap();

    if let Ok(response) = client.get(schema_url).send() {
        if let Ok(schema) = response.json::<Value>() {
            if let Some(schema_props) = schema.get("properties").and_then(|p| p.as_object()) {
                coerce_and_map(&mut clip_parsed, schema_props);
            }
        }
    } else {
        eprintln!("Warning: Could not fetch schema. Skipping coercion.");
    }

    // Capture state before merge
    let old_target = target_data.clone();

    // Perform Merge
    deep_merge(&mut target_data, &clip_parsed);

    // Print the Diff
    println!("\nChanges applied to {}:", target_file);
    let mut changes_found = false;
    print_changes("", &old_target, &target_data, &mut changes_found);

    if !changes_found {
        println!("  (No changes detected. The configurations were already identical.)");
    }
    println!(); // Blank line for clean terminal output

    // Write to file
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).unwrap_or_default();
    }

    let out_json = serde_json::to_string_pretty(&target_data).unwrap();
    if fs::write(&target_path, out_json).is_ok() {
        println!("Success: Configuration written to disk.");
    } else {
        eprintln!("Error: Failed to write to target file.");
        exit(1);
    }
}