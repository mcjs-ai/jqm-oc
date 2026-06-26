# jqm-oc (JSON Query Merge - OpenCode)

`jqm-oc` is a lightning-fast, cross-platform compiled Rust CLI utility designed to safely and dynamically merge JSON/JSONC clipboard data into your local OpenCode configuration files. 

## ✨ New in 1.0.0
* **Safety First:** Auto-backups (`.bak`) created before every write.
* **Dry Run:** Use `--dry-run` or `-d` to simulate merges without disk changes.
* **Alias Registration:** Dynamically alias the tool in your shell: `eval $(jqm-oc --register-alias <name>)`.
* **Custom Schema:** Use `--schema-path` or `-s` to point to any local file or remote URL.

## 🚀 Installation
```bash
# Build the project
git clone [https://github.com/mcjs-ai/jqm-oc.git](https://github.com/mcjs-ai/jqm-oc.git)
cd jqm-oc
cargo build --release

# Move to your bin directory
mkdir -p ~/.local/bin
mv target/release/jqm-oc ~/.local/bin/
```

## 💻 CLI Usage

**Safety & Integrity:**
* **Basic Merge:** `jqm-oc`
* **Dry Run (Simulation):** `jqm-oc --dry-run` or `jqm-oc -d`
* **No Auto-Fix (Strict Mode):** `jqm-oc --no-autofix`

**Advanced Configuration:**
* **Custom Config Path:** `jqm-oc --config-path ./my-config.jsonc` or `jqm-oc -c ./my-config.jsonc`
* **Custom Schema:** `jqm-oc --schema-path ./custom-schema.json` or `jqm-oc -s ./custom-schema.json`
* **Map/Remap Keys:** `jqm-oc --map "oldKey=newKey" --save-map` or `jqm-oc --m "oldKey=newKey" --save-map`
* **Multi-tool Config Merge by registering an alias for a map, schema amd config:** `jqm-oc -s ./tool-schema.json -c ./tool-config.jsonc -m 'keyToCoerce=toolKeyEquivalent,k2=t2,...,kn=tn' --register-alias jqm-tool`


---

### ⌨️ Dynamic Alias Registration (Multi-Tool Workflows)
You can create custom "baked-in" versions of `jqm-oc` using the registration flag. This captures all current flags and parameters, saving them into a reusable alias in your shell. 

This allows you to transform `jqm-oc` into a Swiss Army knife for your entire agentic mesh. Here are practical examples of how to map `jqm-oc` for local development and popular AI coding environments:

#### 1. Project-Specific Configs (The Local Pipeline)
Create a permanent alias for a specific, isolated project configuration. This is perfect for safely testing new Model Context Protocol blocks without touching your global files.
```bash
eval $(jqm-oc -i \
  --config-path ./.opencode/my-local-config.jsonc \
  --register-alias jqm-local)
```
* **Usage:** Type `jqm-local` to open the interactive UI and merge the clipboard exclusively into your local `./my-local-config.jsonc` file.

#### 2. Claude Code (The Anthropic Pipeline)
Map standard MCP keys to Claude's custom format and target its specific config file.
```bash
eval $(jqm-oc -i \
  --config-path ~/.config/claude/config.json \
  --map "mcpServers=mcp,customAgents=claudeAgents" \
  --register-alias jqm-claude)
```
* **Usage:** Type `jqm-claude` to open the interactive UI, apply Claude mappings, and merge into the Claude config.

#### 3. Cursor / Crush (The VS Code Fork Pipeline)
Target the `settings.json` file directly and point to their official schema for remote validation.
```bash
eval $(jqm-oc \
  --config-path ~/.config/Cursor/User/settings.json \
  --schema-path "[https://cursor.com/api/schema.json](https://cursor.com/api/schema.json)" \
  --map "mcpServers=cursor.mcp.servers" \
  --register-alias jqm-crush)
```
* **Usage:** Type `jqm-crush` to instantly merge your clipboard into your Cursor settings, coercing types based on their specific remote schema.

#### 4. Aider (The Terminal Agent Pipeline)
Manage a strict JSON payload file for Aider MCP definitions. Skips the interactive menu and refuses to auto-heal broken syntax.
```bash
eval $(jqm-oc \
  --config-path ~/mcjs-ai/aider-mcp-configs.json \
  --no-autofix \
  --register-alias jqm-aider)
```
* **Usage:** Type `jqm-aider` to instantly and strictly update your local Aider MCP payload.

*(Note: To make these aliases permanent across all terminal sessions, append the command output to your shell's rc file, e.g., `jqm-oc ... --register-alias jqm-claude >> ~/.bashrc`)*


---

## ⌨️ Shell Completions
You can generate completion scripts for Bash, Zsh, Fish, PowerShell, Elvish, and Nushell:

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
echo '$(jqm-oc --generate-completions powershell | Out-String | Invoke-Expression >> $proflile 
# Registers an ArgCompleter for jqm-oc in your profile
```

**Nushell:**
```nushell
jqm-oc --generate-completions nushell | save ~/.config/nushell/jqm-oc-completions.nu
# Add `source ~/.config/nushell/jqm-oc-completions.nu` to your config.nu
```

**Xonsh:**
Xonsh relies on Fish completions via the `fish_completer` extension. 
1. Install the extension: `pip install xontrib-fish-completer`
2. Add to your `~/.xonshrc`: `xontrib load fish_completer`
3. Generate the Fish completion file:
```bash
mkdir -p ~/.config/fish/completions
jqm-oc --generate-completions fish > ~/.config/fish/completions/jqm-oc.fish
```