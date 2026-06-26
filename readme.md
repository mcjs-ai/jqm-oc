# jqm-oc (JSON Query Merge - OpenCode)

`jqm-oc` is a lightning-fast, compiled Rust CLI utility designed to safely and dynamically merge JSON/JSONC clipboard data into your local OpenCode configuration files. 

It completely bypasses the limitations of standard `jq` by offering native JSONC (comments) parsing, dynamic type coercion via the official OpenCode schema, visual terminal diffs, and interactive merging.

## ✨ Features

* **Interactive Cherry-Picking (`-i`):** Interactively select exactly which key->value pairs from your clipboard are merged into your config file.
* **Dynamic Custom Mapping:** Define custom key translations on the fly via CLI flags, and save them persistently to your system.
* **Native JSONC Support:** Safely parses clipboard data and target config files, stripping trailing (`//`) and block (`/* */`) comments.
* **Visual Terminal Diffs:** Automatically prints a color-coded log of exactly which keys were added (`+`) or modified (`~`).
* **Dynamic Schema Coercion:** Fetches the `draft-07` schema from `https://opencode.ai/config.json` and dynamically converts stringified numbers/booleans into their proper schema types.

## 📦 Prerequisites

* **Rust / Cargo** * **xclip** (Linux/X11 clipboard management)

## 🚀 Installation

```bash
git clone [https://github.com/mcjs-ai/jqm-oc.git](https://github.com/mcjs-ai/jqm-oc.git)
cd jqm-oc
cargo build --release
mkdir -p ~/.local/bin
mv target/release/jqm-oc ~/.local/bin/
```

## 💻 CLI Usage

**Basic Merge (Reads clipboard, merges into default OpenCode config):**
```bash
jqm-oc
```

**Interactive Mode:** Opens a terminal UI to check/uncheck nested key->value pairs before merging.
```bash
jqm-oc -i
```

### 🗺️ Custom Alias Mapping

`jqm-oc` includes hardcoded aliases for common MCJS workflows (e.g., `mcpServers` ➔ `mcp`). You can dynamically add to or override these defaults.

* **Use a map for a single run:**
  ```bash
  jqm-oc --map "customTools=tools,legacyNode=node"
  ```
* **Save a map for all future runs:**
  ```bash
  jqm-oc --map "customTools=tools" --save-map
  ```
* **Set/Overwrite the entire saved config:**
  ```bash
  jqm-oc --set-map "myMcp=mcp,myLsp=lsp"
  ```
* **Temporarily ignore the saved map:**
  ```bash
  jqm-oc --no-custom-map
  ```
* **Permanently delete the saved map:**
  ```bash
  jqm-oc --reset-map
  ```