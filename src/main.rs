use anyhow::{bail, Context, Result};
use arboard::Clipboard;
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use dialoguer::{theme::ColorfulTheme, Confirm};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

mod cli;
mod lib;

use cli::{Cli, CompletionShell};
use lib::*;

fn get_map_config_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")).join("jqm-oc/aliases.json")
}

fn load_saved_map() -> HashMap<String, String> {
    let path = get_map_config_path();
    if let Ok(data) = fs::read_to_string(path) {
        if let Ok(map) = serde_json::from_str(&data) { return map; }
    }
    HashMap::new()
}

fn save_map_to_disk(map: &HashMap<String, String>) -> Result<()> {
    let path = get_map_config_path();
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    fs::write(&path, serde_json::to_string_pretty(map)?)?;
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Completions
    if let Some(shell) = cli.generate_completions {
        let mut cmd = Cli::command();
        let bin_name = cmd.get_name().to_string();
        let mut stdout = std::io::stdout();
        match shell {
            CompletionShell::Bash => generate(clap_complete::Shell::Bash, &mut cmd, &bin_name, &mut stdout),
            CompletionShell::Zsh => generate(clap_complete::Shell::Zsh, &mut cmd, &bin_name, &mut stdout),
            CompletionShell::Fish => generate(clap_complete::Shell::Fish, &mut cmd, &bin_name, &mut stdout),
            CompletionShell::Powershell => generate(clap_complete::Shell::PowerShell, &mut cmd, &bin_name, &mut stdout),
            CompletionShell::Elvish => generate(clap_complete::Shell::Elvish, &mut cmd, &bin_name, &mut stdout),
            CompletionShell::Nushell => generate(clap_complete_nushell::Nushell, &mut cmd, &bin_name, &mut stdout),
        }
        return Ok(());
    }

    // Register Alias
    if let Some(alias) = cli.register_alias {
        let args: Vec<String> = std::env::args().collect();
        let mut new_args = Vec::new();
        let mut skip_next = false;
        
        for i in 1..args.len() {
            if skip_next { skip_next = false; continue; }
            if args[i] == "--register-alias" || args[i] == "-r" {
                skip_next = true;
                continue;
            }
            new_args.push(args[i].clone());
        }
        println!("alias {}='jqm-oc {}'", alias, new_args.join(" "));
        return Ok(());
    }

    // Config Reset
    if cli.reset_map {
        let path = get_map_config_path();
        if path.exists() { fs::remove_file(&path)?; println!("Custom alias map reset."); }
        return Ok(());
    }

    // Mapping
    let mut alias_map: HashMap<String, String> = HashMap::from([
        ("mcpServers".to_string(), "mcp".to_string()),
        ("lspServers".to_string(), "lsp".to_string()),
        ("customAgents".to_string(), "agent".to_string()),
    ]);
    if !cli.no_custom_map { alias_map.extend(load_saved_map()); }

    if let Some(ref map_str) = cli.set_map {
        let custom_map = parse_map_string(map_str);
        save_map_to_disk(&custom_map)?;
        alias_map.extend(custom_map);
    } else if let Some(ref map_str) = cli.map {
        let custom_map = parse_map_string(map_str);
        if cli.save_map { save_map_to_disk(&custom_map)?; }
        alias_map.extend(custom_map);
    }

    // Target Path
    let target_file = cli.config_path.unwrap_or_else(|| {
        format!("{}/.config/opencode/opencode.jsonc", std::env::var("HOME").unwrap_or_default())
    });
    let target_path = PathBuf::from(&target_file);

    // Clipboard
    let mut clipboard = Clipboard::new().context("Failed to initialize OS clipboard")?;
    let clip_raw = clipboard.get_text().context("Clipboard is empty")?;
    
    let clean_clip = strip_jsonc(&clip_raw);
    let mut clip_parsed: Value = match serde_json::from_str(&clean_clip) {
        Ok(v) => v,
        Err(e) => {
            if cli.no_autofix { bail!("Invalid JSON: {}", e); }
            let trimmed = clean_clip.trim();
            let mut fixed = String::new();
            if !trimmed.starts_with('{') { fixed.push('{'); }
            fixed.push_str(trimmed);
            if !trimmed.ends_with('}') { fixed.push('}'); }
            if let Ok(val) = serde_json::from_str::<Value>(&fixed) {
                if Confirm::with_theme(&ColorfulTheme::default()).with_prompt("Invalid JSON detected. Heal it?").default(true).interact()? {
                    val
                } else { bail!("Invalid JSON: {}", e); }
            } else { bail!("Invalid JSON: {}", e); }
        }
    };

    if !clip_parsed.is_object() { bail!("Clipboard root must be a JSON Object."); }
    
    let mut target_data: Value = if target_path.exists() {
        serde_json::from_str(&strip_jsonc(&fs::read_to_string(&target_path)?)).unwrap_or_else(|_| Value::Object(std::collections::BTreeMap::new().into_iter().collect()))
    } else { Value::Object(std::collections::BTreeMap::new().into_iter().collect()) };

    if let Some(schema) = fetch_schema(cli.schema_path.as_ref()) {
        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            coerce_and_map(&mut clip_parsed, props, &alias_map);
        }
    }

    if cli.interactive { apply_interactive_mode(&mut clip_parsed)?; }

    let old_target = target_data.clone();
    deep_merge(&mut target_data, &clip_parsed);

    let mut changes = false;
    print_changes("", &old_target, &target_data, &mut changes);

    if !cli.dry_run && changes {
        if target_path.exists() { fs::copy(&target_path, target_path.with_extension("jsonc.bak"))?; }
        if let Some(parent) = target_path.parent() { fs::create_dir_all(parent)?; }
        fs::write(&target_path, serde_json::to_string_pretty(&target_data)?)?;
        println!("Success: Configuration written (backup created).");
    } else if cli.dry_run {
        println!("Dry run complete. No changes written.");
    } else {
        println!("No changes detected.");
    }

    Ok(())
}
