# qwen_cli_assistant

Ultra-safe CLI assistant powered by a local Ollama model (e.g. `qwen2.5-coder:7b`).

Features:

- Natural language → shell command suggestion.
- Ultra-safe mode (default): blocks dangerous commands (`rm -rf /`, `mkfs`, `dd` on disks, etc).
- Explicit confirmation before running anything.
- Per-session chat history so the model can refine commands.
- Optional agent mode: multi-step plans (each step previewed and confirmed).
- Optional script mode: generate a Bash script file from a description.
- Optional clipboard copy of the suggested command.
- Intelligent caching with automatic invalidation and semantic matching.
- Designed for use with zsh / bash and local Ollama.

## Caching

The assistant uses intelligent caching to improve performance and reduce API calls:

- **Automatic Invalidation**: Cache entries expire after 7 days
- **Semantic Matching**: Understands similar requests even with different wording (e.g., "show disk space" matches "check free space")
- **Manual Clearing**: Use `--retrain` to clear cache and start fresh
- **Cache Location**: Stored at `~/.config/qwen_cli_assistant/cache.json`

Cache commands:
```bash
# Clear cache and retrain
qwen-cli --retrain

# View cached commands
cat ~/.config/qwen_cli_assistant/cache.json
```

## Requirements

- Rust toolchain (cargo, rustc).
- Ollama running locally, for example:

```bash
ollama serve
ollama pull qwen2.5-coder:7b
```

Or if using Docker, expose the API at `http://localhost:11434`.

## Build

```bash
cd qwen_cli_assistant
cargo build --release
```

The binary will be at:

```bash
target/release/qwen_cli_assistant
```

You can then move or symlink it into your PATH, e.g.:

```bash
sudo mv target/release/qwen_cli_assistant /usr/local/bin/qwen-cli
```

## Usage

One-shot (single prompt):

```bash
qwen-cli "find all .rs files larger than 1MB"
```

Interactive chat:

```bash
qwen-cli --chat
```

Agent (multi-step plan):

```bash
qwen-cli --agent "clean Cargo build artifacts and summarize disk usage"
```

Script generation:

```bash
qwen-cli --script -o clean_project.sh "remove target folders and temporary files safely"
```

Cache management:

```bash
# Clear cache and start fresh
qwen-cli --retrain "find large files in current directory"
```

Safe vs unsafe:

- Default is **ultra-safe**: blocks `sudo`, destructive disk ops, `rm -rf /`, etc.
- You can relax some checks with `--unsafe`, but **still** every command is shown and must be confirmed.

## Optional zsh keybinding

For zsh, you can add something like this to your `.zshrc`:

```zsh
qwen_cli_widget() {
  BUFFER="qwen-cli --chat"
  zle accept-line
}
zle -N qwen_cli_widget
bindkey '^G' qwen_cli_widget
```

Then press `Ctrl-G` in the terminal to open an interactive assistant session.
