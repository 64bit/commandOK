# commandok

Spotlight-like command generator for your terminal. Pops up when you need it and gets out of the way when you don't.

Describe what you want in plain English, get the shell command back instantly.

Built with [Ratatui](https://ratatui.rs) and powered by your choice of LLM provider.


## Demo

```
╭ ⌘OK · anthropic(claude-opus-4-6) ───────────────────────────────╮
│ > find all rust files modified in the last 24 hours             │
╰──────────────── ↵ submit · ⇧Tab switch · esc cancel ────────────╯

╭ ⌘OK · anthropic(claude-opus-4-6) ───────────────────────────────╮
│ $ find . -name "*.rs" -mtime -1                                 │
╰──────────────────────────── ↵ accept · esc cancel ──────────────╯
```

**WARN**: you must always verify the generated command before accepting it

## Install

```bash
cargo install commandok
```

## Setup

On first run, a default config is created at `~/.commandok/config.toml`. Add your API key for at least one provider:

```toml
[commandok]
provider = "anthropic"  # Options: anthropic, openai, google, ollama
system_prompt = "You are a terminal command generator. Given a natural language description, output ONLY the shell command appropriate for the user's OS and shell. No explanation, no markdown, no code blocks, no backticks. Just the raw command."

[anthropic]
api_key = "sk-ant-..."
model = "claude-opus-4-6"

[openai]
api_key = "sk-..."
model = "gpt-5.4"

[google]
api_key = "..."
model = "gemini-3-flash-preview"

[ollama]
model = "gemma3:1b"
# api_url = "http://localhost:11434"  # default, change if running elsewhere

[openrouter]
api_key = ""
model = "qwen/qwen3.6-plus:free"
# api_url = "https://openrouter.ai/api/v1"  # default
```

## Usage

Run `commandok` in any terminal. A search bar appears inline below your cursor.

1. Type a natural language description of the command you need
2. Press **Enter** -- the command streams in token-by-token
3. Press **Enter** again to accept (injects the command into your shell) or **Esc** to cancel

### Keybindings

| Key | Action |
|-----|--------|
| **Enter** | Submit prompt / Accept generated command |
| **Esc** | Cancel and dismiss |
| **Shift+Tab** | Cycle through configured providers |
| **Up / Down** | Browse prompt history |
| **Ctrl+U** | Clear input line |
| **Ctrl+C** | Quit |
| **Left / Right / Home / End** | Move cursor |


## Adding a new provider

1. Create `src/provider/yourprovider.rs` with a `pub async fn stream(...)` function
2. Add `pub mod yourprovider;` to `src/provider/mod.rs`
3. Add a variant to the `Provider` enum and wire it in `from_name()` / `stream()`
4. Add the section to `Config` in `src/config.rs` and `PROVIDER_ORDER`

## License

MIT
