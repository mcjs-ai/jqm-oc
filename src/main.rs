use anyhow::{bail, Context, Result};
use arboard::Clipboard;
use clap::{CommandFactory, Parser, ValueEnum};
use clap_complete::{generate, Shell};
use clap_complete_nushell::Nushell;
use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect};
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

#[derive(ValueEnum, Clone)]
enum CompletionShell {
    Bash, Zsh, Fish, Powershell, Elvish, Nushell,
}

#[derive(Parser)]
#[command(author, version, about = "JSON Query Merge for OpenCode", long_about = None)]
struct Cli {
    /// The target jsonc file (defaults to ~/.config/opencode/opencode.jsonc)
    target_file: Option<String>,

    /// Interactively select keys or key->value pairs to add
    #[arg(short, long)]
    interactive: bool,

    /// Suppress the auto-fix prompt for missing JSON braces
    #[arg(long)]
    no_autofix: bool,

    /// Provide a custom map (format: oldKey=newKey,o2=n2)
    #[arg(long)]
    map: Option<String>,

    /// Save the provided --map to the config file for future use
    #[arg(long, requires = "map")]
    save_map: bool,

    /// Set and save a custom map, overwriting the existing config
    #[arg(long, conflicts_with_all = ["map", "reset_map", "no_custom_map"])]
    set_map: Option<String>,

    /// Ignore any saved custom map in the config file
    #[arg(long, conflicts_with_all = ["map", "set_map", "reset_map"])]
    no_custom_map: bool,

    /// Clear/reset the saved custom map in the config file
    #[arg(long, conflicts_with_all = ["map", "set_map", "no_custom_map"])]
    reset_map: bool,

    /// Generate shell completion scripts
    #[arg(long, value_enum, hide = true)]
    generate_completions: Option<CompletionShell>,
}

// --- Configuration & Paths ---

fn get_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(env::var("HOME").unwrap_or_default()).join(".config"))
        .join("jqm-oc")
}

fn get_map_config_path() -> PathBuf {
    get_config_dir().join("aliases.json")
}

fn get_schema_cache_path() -> PathBuf {
    get_config_dir().join("schema_cache.json")
}

// --- Map Management ---

fn load_saved_map() -> HashMap<String, String> {
    let path = get_map_config_path();
    if let Ok(data) = fs::read_to_string(path) {
        if let Ok(map) = serde_json::from_str(&data) {
            return map;
        }
    }
    HashMap::new()
}

fn save_map_to_disk(map: &HashMap<String, String>) -> Result<()> {
    let path = get_map_config_path();
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    let json = serde_json::to_string_pretty(map)?;
    fs::write(&path, json).context("Failed to save map config to disk")?;
    println!("Saved custom alias map to {:?}", path);
    Ok(())
}

fn parse_map_string(s: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in s.split(',') {
        if let Some((old, new)) = pair.split_once('=') {
            map.insert(old.trim().to_string(), new.trim().to_string());
        }
    }
    map
}

// --- JSON Helpers ---

fn strip_jsonc(input: &str) -> String {
    let re = Regex::new(r#"(?s)("(?:\\.|[^"\\])*")|(//[^\n]*|/\*.*?\*/)"#).unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        if let Some(string_match) = caps.get(1) { string_match.as_str().to_string() } else { String::new() }
    }).to_string()
}

fn deep_merge(target: &mut Value, source: &Value) {
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

// --- Schema & Coercion ---

fn fetch_or_load_schema() -> Option<Value> {
    let cache_path = get_schema_cache_path();
    let cache_duration = Duration::from_secs(60 * 60 * 24); // 24 hours

    // Try reading valid cache first
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

    // Fetch from web if cache is missing or stale
    let schema_url = "https://opencode.ai/config.json";
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(3)).build().ok()?;
    
    if let Ok(response) = client.get(schema_url).send() {
        if let Ok(schema) = response.json::<Value>() {
            // Save cache invisibly
            if let Some(parent) = cache_path.parent() { let _ = fs::create_dir_all(parent); }
            let _ = fs::write(&cache_path, serde_json::to_string(&schema).unwrap_or_default());
            return Some(schema);
        }
    }
    None
}

fn coerce_and_map(clip: &mut Value, schema_props: &Map<String, Value>, alias_map: &HashMap<String, String>) {
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

// --- Interactive Selection ---

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

fn apply_interactive_mode(clip_parsed: &mut Value) -> Result<()> {
    let mut leaves = Vec::new();
    flatten_to_leaves(clip_parsed, "", &mut leaves);
    if leaves.is_empty() { bail!("Clipboard object is empty or invalid for interactive selection."); }

    let display_strings: Vec<String> = leaves.iter().map(|(path, val)| format!("{} = {}", path, val)).collect();
    let defaults: Vec<bool> = vec![true; leaves.len()];

    println!("\n\x1b[36mInteractive Mode: Select the key->value pairs to merge into OpenCode.\x1b[0m");
    let selection_indices = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Use Space to toggle, Enter to confirm")
        .items(&display_strings)
        .defaults(&defaults)
        .interact()?;

    if selection_indices.is_empty() { bail!("No keys selected. Aborting merge."); }

    let selected_leaves: Vec<(String, Value)> = selection_indices.into_iter().map(|i| leaves[i].clone()).collect();
    *clip_parsed = unflatten_leaves(selected_leaves);
    Ok(())
}

// --- Printing Diffs ---

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

// --- Core Flow ---

fn run() -> Result<()> {
    let cli = Cli::parse();

    // 1. Generate Completions
    if let Some(shell) = cli.generate_completions {
        let mut cmd = Cli::command();
        let bin_name = cmd.get_name().to_string();
        let mut stdout = std::io::stdout();
        match shell {
            CompletionShell::Bash => generate(Shell::Bash, &mut cmd, &bin_name, &mut stdout),
            CompletionShell::Zsh => generate(Shell::Zsh, &mut cmd, &bin_name, &mut stdout),
            CompletionShell::Fish => generate(Shell::Fish, &mut cmd, &bin_name, &mut stdout),
            CompletionShell::Powershell => generate(Shell::PowerShell, &mut cmd, &bin_name, &mut stdout),
            CompletionShell::Elvish => generate(Shell::Elvish, &mut cmd, &bin_name, &mut stdout),
            CompletionShell::Nushell => generate(Nushell, &mut cmd, &bin_name, &mut stdout),
        }
        return Ok(());
    }

    // 2. Handle Config Resets
    if cli.reset_map {
        let path = get_map_config_path();
        if path.exists() { fs::remove_file(&path)?; println!("Custom alias map reset."); }
        return Ok(());
    }

    // 3. Build Mapping Dictionary
    let mut alias_map: HashMap<String, String> = HashMap::from([
        ("mcpServers".to_string(), "mcp".to_string()),
        ("lspServers".to_string(), "lsp".to_string()),
        ("customAgents".to_string(), "agent".to_string()),
    ]);

    let mut custom_map = HashMap::new();
    if !cli.no_custom_map { custom_map.extend(load_saved_map()); }

    if let Some(ref map_str) = cli.set_map {
        custom_map = parse_map_string(map_str);
        save_map_to_disk(&custom_map)?;
    } else if let Some(ref map_str) = cli.map {
        custom_map.extend(parse_map_string(map_str));
        if cli.save_map { save_map_to_disk(&custom_map)?; }
    }
    alias_map.extend(custom_map);

    // 4. Resolve Target File
    let target_file = cli.target_file.unwrap_or_else(|| {
        format!("{}/.config/opencode/opencode.jsonc", env::var("HOME").unwrap_or_default())
    });
    let target_path = PathBuf::from(&target_file);

    // 5. Read Clipboard safely
    let mut clipboard = Clipboard::new().context("Failed to initialize OS clipboard")?;
    let clip_raw = clipboard.get_text().context("Clipboard is empty or contains non-text data")?;
    if clip_raw.trim().is_empty() { bail!("Clipboard is empty."); }

    let clean_clip = strip_jsonc(&clip_raw);
    
    // 6. Parse and Auto-Heal JSON
    let mut clip_parsed: Value = match serde_json::from_str(&clean_clip) {
        Ok(v) => v,
        Err(e) => {
            if cli.no_autofix { bail!("Invalid JSON/JSONC.\n{}", e); }
            
            let trimmed = clean_clip.trim();
            let mut fixed_clip = String::new();
            let mut changed = false;

            if !trimmed.starts_with('{') { fixed_clip.push('{'); changed = true; }
            fixed_clip.push_str(trimmed);
            if !trimmed.ends_with('}') { fixed_clip.push('}'); changed = true; }

            if changed {
                if let Ok(fixed_val) = serde_json::from_str::<Value>(&fixed_clip) {
                    println!("\x1b[33mWarning: Malformed JSON detected (missing root braces).\x1b[0m");
                    println!("Proposed fix:\n{}", fixed_clip);
                    
                    let apply = Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt("Do you wish to correct it and continue?")
                        .default(true)
                        .interact()?;

                    if apply { fixed_val } else { bail!("Aborted by user."); }
                } else { bail!("Invalid JSON/JSONC. Auto-fix failed.\n{}", e); }
            } else { bail!("Invalid JSON/JSONC.\n{}", e); }
        }
    };

    // 7. Verify Root Object Integrity
    if !clip_parsed.is_object() {
        bail!("Clipboard data MUST be a valid JSON Object at its root.\nAborting to prevent full-file overwrite.");
    }

    // 8. Parse Target File
    let mut target_data: Value = if target_path.exists() {
        let raw_target = fs::read_to_string(&target_path).unwrap_or_default();
        serde_json::from_str(&strip_jsonc(&raw_target)).unwrap_or_else(|_| Value::Object(Map::new()))
    } else { Value::Object(Map::new()) };

    // 9. Fetch Schema and Map/Coerce
    if let Some(schema) = fetch_or_load_schema() {
        if let Some(schema_props) = schema.get("properties").and_then(|p| p.as_object()) {
            coerce_and_map(&mut clip_parsed, schema_props, &alias_map);
        }
    }

    // 10. Optional Interactive Cherry-Picking
    if cli.interactive {
        apply_interactive_mode(&mut clip_parsed)?;
    }

    // 11. Perform the Deep Merge
    let old_target = target_data.clone();
    deep_merge(&mut target_data, &clip_parsed);

    // 12. Display Diff and Write
    println!("\nChanges applied to {}:", target_file);
    let mut changes_found = false;
    print_changes("", &old_target, &target_data, &mut changes_found);

    if !changes_found {
        println!("  (No changes detected.)\n");
        return Ok(());
    }

    if let Some(parent) = target_path.parent() { fs::create_dir_all(parent)?; }
    fs::write(&target_path, serde_json::to_string_pretty(&target_data)?)
        .context("Failed to write to target file")?;

    println!("\nSuccess: Configuration written to disk.");
    Ok(())
}

// Minimal main wrapper to cleanly print anyhow errors
fn main() {
    if let Err(e) = run() {
        eprintln!("\x1b[31mError:\x1b[0m {:#}", e);
        std::process::exit(1);
    }
}