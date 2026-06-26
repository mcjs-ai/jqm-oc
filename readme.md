# jqm-oc (JSON Query Merge - OpenCode)

`jqm-oc` is a lightning-fast, compiled Rust CLI utility designed to safely and dynamically merge JSON/JSONC clipboard data into your local OpenCode configuration files. 

It completely bypasses the limitations of standard `jq` by offering native JSONC (comments) parsing, dynamic type coercion via the official OpenCode schema, and automatic key mapping for legacy or alternative configuration blocks (such as Model Context Protocol definitions for MCJS operations).

## ✨ Features

* **Native JSONC Support:** Safely parses both your clipboard data and your target config files, stripping trailing (`//`) and block (`/* */`) comments without breaking the parser.
* **Dynamic Schema Coercion:** Automatically fetches the `draft-07` schema directly from `https://opencode.ai/config.json`. If you copy stringified values (e.g., `"true"` or `"5"`), it dynamically converts them into their proper schema types (`boolean`, `number`) before saving.
* **Intelligent Key Translation:** Automatically intercepts and maps alternative keys to their official OpenCode schema equivalents:
    * `mcpServers` ➔ `mcp` (Specifically mapped to handle Model Context Protocol configurations seamlessly)
    * `lspServers` ➔ `lsp`
    * `customAgents` ➔ `agent`
* **Recursive Deep Merging:** Preserves existing configurations by intelligently deep-merging nested objects rather than blindly overwriting root keys.
* **Atomic Writes:** Ensures your config file is never corrupted; it only writes to disk if the clipboard parsing, mapping, and merging succeed entirely.

## 📦 Prerequisites

* **Rust / Cargo:** To compile the binary natively.
* **xclip:** Used to seamlessly read from the system clipboard on Linux/X11.

## 🚀 Installation

1. Clone the repository:
   ```bash
   git clone [https://github.com/mcjs-ai/jqm-oc.git](https://github.com/mcjs-ai/jqm-oc.git)
   cd jqm-oc
   ```
2. Compile the release binary:
   ```bash
   cargo build --release
   ```
3. Move the executable to your local bin path:
   ```bash
   mkdir -p ~/.local/bin
   mv target/release/jqm-oc ~/.local/bin/
   ```
4. Ensure `~/.local/bin` is in your shell's `$PATH`.

## 💻 Usage

Simply copy any valid JSON or JSONC block to your clipboard, and run the command.

**Default Behavior:**
If run without arguments, it defaults to modifying `~/.config/opencode/opencode.jsonc`.
```bash
jqm-oc
```

**Custom Target:**
You can also pass a specific path as an argument to target different configuration files.
```bash
jqm-oc /path/to/another/config.jsonc
```

### Example Workflow

1. Copy the following text to your clipboard:
```jsonc
{
    // This is my legacy Model Context Protocol block
    "mcpServers": {
        "my-server": {
            "command": "node",
            "alwaysAllow": "true" // This is a string, but the schema wants a boolean
        }
    }
}
```

2. Run `jqm-oc`.

3. The tool fetches the schema, maps `mcpServers` to `mcp`, coerces `"true"` to `true`, deep-merges it into your existing OpenCode file, and saves the result perfectly formatted.

## 🛠️ Modifying Key Maps

To add new key aliases (e.g., mapping `"customTools"` to `"tools"`), open `src/main.rs`, locate the `coerce_and_map` function, and add your mapping to the `match` statement:

```rust
let final_key = match k.as_str() {
    "mcpServers" => "mcp",
    "lspServers" => "lsp",
    "customAgents" => "agent",
    "customTools" => "tools", // New mapping
    other => other,
};
```
Recompile with `cargo build --release` to apply changes.