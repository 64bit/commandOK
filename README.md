# ⌘OK

![Demo of commandOK in action](https://raw.githubusercontent.com/64bit/commandOK/refs/heads/main/commandok.gif)

# commandOK

**commandOK** is a Spotlight-like command generator for your terminal. Pops up when you need it and gets out of the way when you don't.

Built with [Ratatui](https://ratatui.rs) and powered by your choice of public, private or local LLM provider.

**WARN**: you must always verify the generated command before accepting it

## Install

```bash
brew install 64bit/tap/commandok
```

OR

```bash
cargo install commandok
```

## Setup

On first run, a default config is created at `~/.commandok/config.toml`. Add your API key for at least one provider:

```toml
[commandok]
# Options: anthropic, openai, google, mistral, ollama,
#          openrouter, xai, vercel_ai_gateway, litert_lm,
#          apple_intelligence (requires building with --features apple-intelligence on macOS 26+ ARM)
provider = "anthropic"
system_prompt = """\
You are a terminal command generator. Given a natural language description, output ONLY \
the shell command appropriate for the user's OS and shell. No explanation, no markdown, no code blocks, \
no backticks. Just the raw command.\
"""

[anthropic]
api_key = ""
model = "claude-opus-4-6"

[openai]
api_key = ""
model = "gpt-5.4"

[google]
api_key = ""
model = "gemini-3-flash-preview"

[mistral]
api_key = ""
model = "mistral-small-latest"
# api_url = "https://api.mistral.ai/v1"  # default

[ollama]
model = "gemma3:1b"
# api_url = "http://localhost:11434"  # default, change if running elsewhere

[openrouter]
api_key = ""
model = "qwen/qwen3.6-plus:free"
# api_url = "https://openrouter.ai/api/v1"  # default

[xai]
api_key = ""
model = "grok-4.20-0309-reasoning"
# api_url = "https://api.x.ai/v1"  # default

[vercel_ai_gateway]
api_key = ""
model = "google/gemini-3-flash"
# api_url = "https://ai-gateway.vercel.sh/v1"  # default

[litert_lm]
model = "gemma-4-E2B-it.litertlm"
huggingface_repo = "litert-community/gemma-4-E2B-it-litert-lm"

[apple_intelligence]
model = "system"
```

## Apple Intelligence (optional)

On macOS 26+ on Apple Silicon, commandOK can run prompts entirely on-device through Apple's FoundationModels framework. It is gated behind a Cargo feature so the default install does not require the Swift toolchain.

```bash
cargo install commandok --features apple-intelligence
```

Building the feature requires the Xcode Command Line Tools (`xcode-select --install`). At runtime, Apple Intelligence must be enabled in System Settings.

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
