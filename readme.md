# jqm-oc (JSON Query Merge - OpenCode)

`jqm-oc` is a lightning-fast, cross-platform compiled Rust CLI utility designed to safely and dynamically merge JSON/JSONC clipboard data into your local OpenCode configuration files. 

It completely bypasses the limitations of standard `jq` by offering native JSONC (comments) parsing, auto-healing for malformed clips, dynamic type coercion via the official OpenCode schema, visual terminal diffs, interactive merging, and dynamic shell completions.

## ✨ Features

* **JSON Auto-Fix Healing:** Automatically detects and heals incomplete JSON copies (e.g., missing a starting `{` or trailing `}`) and prompts for confirmation.
* **Cross-Platform Clipboard Integration:** Native support for Windows, macOS, and Linux out of the box using the `arboard` backend.
* **Failsafe Root Protection:** Strictly prevents accidental full-file overwrites if you mistakenly copy arrays or raw strings.
* **Interactive Cherry-Picking (`-i`):** Interactively select exactly which key->value pairs from your clipboard are merged into your config file.
* **Dynamic Custom Mapping:** Define custom key translations on the fly via CLI flags, and save them persistently to your system.
* **Dynamic Shell Completions:** Auto-generate native completion scripts for Bash, Zsh, Fish, PowerShell, Elvish, and Nushell.
* **Native JSONC Support:** Safely parses clipboard data and target config files, stripping comments.
* **Visual Terminal Diffs:** Prints a color-coded log of exactly which keys were added (`+`) or modified (`~`).

## 📦 Prerequisites

* **Rust / Cargo** (The tool is self-contained after compilation; no other runtime dependencies are required).

## 🚀 Installation

### 1. Build from Source
```bash
git clone [https://github.com/mcjs-ai/jqm-oc.git](https://github.com/mcjs-ai/jqm-oc.git)
cd jqm-oc
cargo build --release
```

### 2. OS-Specific Setup

**Linux:**
```bash
mkdir -p ~/.local/bin
mv target/release/jqm-oc ~/.local/bin/
# Ensure ~/.local/bin is in your $PATH
```

**macOS:**
```bash
# Move to a system path (requires sudo or permissions)
sudo mv target/release/jqm-oc /usr/local/bin/
```

**Windows 11:**
1. Create a folder in your user directory, e.g., `C:\Users\<YourUsername>\bin`.
2. Move `target\release\jqm-oc.exe` into that folder.
3. Open **Environment Variables** (search in Start menu).
4. Select `Path` -> `Edit` -> `New` -> Add `C:\Users\<YourUsername>\bin`.
5. Restart your terminal (PowerShell or CMD) to apply.

## 💻 CLI Usage

**Basic Merge:**
```bash
jqm-oc
```

**Interactive Mode:**
```bash
jqm-oc -i
```

**Disable Auto-Fix Heuristics:**
```bash
jqm-oc --no-autofix
```

### 🗺️ Custom Alias Mapping

`jqm-oc` includes hardcoded aliases for common MCJS workflows (e.g., `mcpServers` ➔ `mcp`).

* **Use a map for a single run:** `jqm-oc --map "customTools=tools,legacyNode=node"`
* **Save a map for all future runs:** `jqm-oc --map "customTools=tools" --save-map`
* **Set/Overwrite the saved map:** `jqm-oc --set-map "myMcp=mcp,myLsp=lsp"`
* **Permanently delete the saved map:** `jqm-oc --reset-map`

### ⌨️ Shell Completions

`jqm-oc` generates its own autocomplete scripts on the fly.

**Bash:**
```bash
mkdir -p ~/.local/share/bash-completion/completions
jqm-oc --generate-completions bash > ~/.local/share/bash-completion/completions/jqm-oc
```

**Zsh:**
```zsh
jqm-oc --generate-completions zsh > ~/.zfunc/_jqm-oc
# Ensure fpath+=~/.zfunc and compinit are in your .zshrc
```

**Fish:**
```fish
jqm-oc --generate-completions fish > ~/.config/fish/completions/jqm-oc.fish
```

**PowerShell:**
```powershell
jqm-oc --generate-completions powershell > "$PROFILE\..\jqm-oc-completion.ps1"
# Add . "$PROFILE\..\jqm-oc-completion.ps1" to your profile
```

**Nushell:**
```nushell
jqm-oc --generate-completions nushell | save ~/.config/nushell/jqm-oc-completions.nu
# Add `source ~/.config/nushell/jqm-oc-completions.nu` to your config.nu
```

**Xonsh:**
1. `pip install xontrib-fish-completer`
2. Add `xontrib load fish_completer` to `~/.xonshrc`
3. Run: `jqm-oc --generate-completions fish > ~/.config/fish/completions/jqm-oc.fish`