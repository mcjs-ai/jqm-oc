use clap::Parser;
use dialoguer::{theme::ColorfulTheme, MultiSelect};
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{exit, Command};

#[derive(Parser)]
#[command(author, version, about = "JSON Query Merge for OpenCode", long_about = None)]
struct Cli {
    /// The target jsonc file (defaults to ~/.config/opencode/opencode.jsonc)
    target_file: Option<String>,

    /// Interactively select keys or key->value pairs to add
    #[arg(short, long)]
    interactive: bool,

    /// Provide a custom map (format: oldKey=newKey,o2=n2)
    #[arg(long)]
    map: Option<String>,

    /// Save the provided --map to the config file for future use
    #[arg(long)]
    save_map: bool,

    /// Set and save a custom map, overwriting the existing config (format: oldKey=newKey)
    #[arg(long)]
    set_map: Option<String>,

    /// Ignore any saved custom map in the config file
    #[arg(long)]
    no_custom_map: bool,

    /// Clear/reset the saved custom map in the config file
    #[arg(long)]
    reset_map: bool,
}

// Map configuration path: ~/.config/jqm-oc/aliases.json
fn get_map_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(env::var("HOME").unwrap_or_default()).join(".config"))
        .join("jqm-oc/aliases.json")
}

fn load_saved_map() -> HashMap<String, String> {
    let path = get_map_config_path();
    if path.exists() {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(map) = serde_json::from_str(&data) {
                return map;
            }
        }
    }
    HashMap::new()
}

fn save_map_to_disk(map: &HashMap<String, String>) {
    let path = get_map_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_default();
    }
    let json = serde_json::to_string_pretty(map).unwrap();
    fs::write(&path, json).unwrap_or_else(|_| eprintln!("Warning: Failed to save map config."));
    println!("Saved custom alias map to {:?}", path);
}

fn parse_map_string(s: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in s.split(',') {
        let parts: Vec<&str> = pair.split('=').collect();
        if parts.len() == 2 {
            map.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
        }
    }
    map
}

fn strip_jsonc(input: &str) -> String {
    let re = Regex::new(r#"(?s)("(?:\\.|[^"\\])*")|(//[^\n]*|/\*.*?\*/)"#).unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        if let Some(string_match) = caps.get(1) {
            string_match.as_str().to_string()
        } else {
            String::new()
        }
    }).to_string()
}

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

fn coerce_and_map(clip: &mut Value, schema_props: &Map<String, Value>, alias_map: &HashMap<String, String>) {
    let obj = match clip.as_object_mut() {
        Some(o) => o,
        None => return,
    };

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

// Flatten JSON to dot-notation leaf nodes for interactive selection
fn flatten_to_leaves(val: &Value, prefix: &str, acc: &mut Vec<(String, Value)>) {
    match val {
        Value::Object(map) => {
            for (k, v) in map {
                let new_prefix = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
                flatten_to_leaves(v, &new_prefix, acc);
            }
        }
        _ => acc.push((prefix.to_string(), val.clone())), // Arrays treated as leaves
    }
}

// Rebuild JSON from dot-notation paths
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

fn print_changes(path: &str, old: &Value, new: &Value, changes_found: &mut bool) {
    match (old, new) {
        (Value::Object(o1), Value::Object(o2)) => {
            for (k, v2) in o2 {
                let new_path = if path.is_empty() { k.clone() } else { format!("{}.{}", path, k) };
                if let Some(v1) = o1.get(k) {
                    if v1 != v2 { print_changes(&new_path, v1, v2, changes_found); }
                } else {
                    *changes_found = true;
                    println!("  \x1b[32m+\x1b[0m {} = {}", new_path, v2);
                }
            }
        },
        (Value::Array(a1), Value::Array(a2)) => {
            if a1 != a2 {
                *changes_found = true;
                println!("  \x1b[33m~\x1b[0m {} (Array updated: {} -> {} items)", path, a1.len(), a2.len());
            }
        },
        (v1, v2) => {
            if v1 != v2 {
                *changes_found = true;
                println!("  \x1b[33m~\x1b[0m {} : {} -> {}", path, v1, v2);
            }
        }
    }
}

fn main() {
    let cli = Cli::parse();

    // Handle map resetting
    if cli.reset_map {
        let path = get_map_config_path();
        if path.exists() {
            fs::remove_file(&path).unwrap_or_default();
            println!("Custom alias map reset.");
        }
        exit(0);
    }

    // Build the dynamic alias map
    let mut alias_map: HashMap<String, String> = HashMap::from([
        ("mcpServers".to_string(), "mcp".to_string()),
        ("lspServers".to_string(), "lsp".to_string()),
        ("customAgents".to_string(), "agent".to_string()),
    ]);

    let mut custom_map = HashMap::new();

    if !cli.no_custom_map {
        custom_map.extend(load_saved_map());
    }

    if let Some(ref map_str) = cli.set_map {
        custom_map = parse_map_string(map_str);
        save_map_to_disk(&custom_map);
    } else if let Some(ref map_str) = cli.map {
        let parsed = parse_map_string(map_str);
        custom_map.extend(parsed);
        if cli.save_map {
            save_map_to_disk(&custom_map);
        }
    }

    alias_map.extend(custom_map);

    let target_file = cli.target_file.unwrap_or_else(|| {
        let home = env::var("HOME").expect("HOME environment variable not set");
        format!("{}/.config/opencode/opencode.jsonc", home)
    });

    let target_path = PathBuf::from(&target_file);

    let clip_output = Command::new("xclip").args(["-selection", "clipboard", "-o"]).output()
        .unwrap_or_else(|_| { eprintln!("Error: Failed to execute xclip."); exit(1); });

    let clip_raw = String::from_utf8_lossy(&clip_output.stdout).to_string();
    if clip_raw.trim().is_empty() {
        eprintln!("Error: Clipboard is empty.");
        exit(1);
    }

    let mut target_data: Value = if target_path.exists() {
        let raw_target = fs::read_to_string(&target_path).unwrap_or_default();
        serde_json::from_str(&strip_jsonc(&raw_target)).unwrap_or_else(|_| Value::Object(Map::new()))
    } else {
        Value::Object(Map::new())
    };

    let mut clip_parsed: Value = serde_json::from_str(&strip_jsonc(&clip_raw))
        .unwrap_or_else(|e| { eprintln!("Error: Invalid JSON/JSONC.\n{}", e); exit(1); });

    let schema_url = "https://opencode.ai/config.json";
    let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(3)).build().unwrap();

    if let Ok(response) = client.get(schema_url).send() {
        if let Ok(schema) = response.json::<Value>() {
            if let Some(schema_props) = schema.get("properties").and_then(|p| p.as_object()) {
                coerce_and_map(&mut clip_parsed, schema_props, &alias_map);
            }
        }
    }

    // --- INTERACTIVE MODE ---
    if cli.interactive {
        let mut leaves = Vec::new();
        flatten_to_leaves(&clip_parsed, "", &mut leaves);

        if leaves.is_empty() {
            eprintln!("Clipboard object is empty or invalid.");
            exit(0);
        }

        let display_strings: Vec<String> = leaves.iter()
            .map(|(path, val)| format!("{} = {}", path, val))
            .collect();
        
        let defaults: Vec<bool> = vec![true; leaves.len()];

        println!("\n\x1b[36mInteractive Mode: Select the key->value pairs to merge into OpenCode.\x1b[0m");
        let selection_indices = MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Use Space to toggle, Enter to confirm")
            .items(&display_strings)
            .defaults(&defaults)
            .interact()
            .unwrap();

        if selection_indices.is_empty() {
            println!("No keys selected. Aborting merge.");
            exit(0);
        }

        let selected_leaves: Vec<(String, Value)> = selection_indices.into_iter()
            .map(|i| leaves[i].clone())
            .collect();

        clip_parsed = unflatten_leaves(selected_leaves);
    }

    let old_target = target_data.clone();
    deep_merge(&mut target_data, &clip_parsed);

    println!("\nChanges applied to {}:", target_file);
    let mut changes_found = false;
    print_changes("", &old_target, &target_data, &mut changes_found);

    if !changes_found {
        println!("  (No changes detected.)\n");
        exit(0);
    }
    println!();

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).unwrap_or_default();
    }
    fs::write(&target_path, serde_json::to_string_pretty(&target_data).unwrap())
        .unwrap_or_else(|_| { eprintln!("Error: Failed to write to target."); exit(1); });

    println!("Success: Configuration written to disk.");
}