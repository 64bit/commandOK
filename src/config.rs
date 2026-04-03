use std::fs;
use std::io::Write;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub commandok: CommandokConfig,
    pub anthropic: Option<ProviderConfig>,
    pub openai: Option<ProviderConfig>,
    pub google: Option<ProviderConfig>,
    pub mistral: Option<ProviderConfig>,
    pub ollama: Option<ProviderConfig>,
    pub openrouter: Option<ProviderConfig>,
    pub xai: Option<ProviderConfig>,
}

#[derive(Deserialize)]
pub struct CommandokConfig {
    pub provider: String,
    pub system_prompt: String,
}

#[derive(Deserialize, Clone)]
pub struct ProviderConfig {
    #[serde(default)]
    pub api_key: String,
    pub model: String,
    #[serde(default)]
    pub api_url: String,
}

fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".commandok")
}

fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

const DEFAULT_CONFIG: &str = r#"[commandok]
provider = "anthropic"  # Options: anthropic, openai, google, mistral, ollama, openrouter, xai
system_prompt = "You are a terminal command generator. Given a natural language description, output ONLY the shell command appropriate for the user's OS and shell. No explanation, no markdown, no code blocks, no backticks. Just the raw command."

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
"#;

pub fn load() -> Result<Config, String> {
    let path = config_path();

    if !path.exists() {
        let dir = config_dir();
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create {}: {e}", dir.display()))?;
        fs::File::create(&path)
            .and_then(|mut f| f.write_all(DEFAULT_CONFIG.as_bytes()))
            .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;

    let config: Config =
        toml::from_str(&content).map_err(|e| format!("Invalid config {}: {e}", path.display()))?;

    Ok(config)
}

const PROVIDER_ORDER: &[&str] = &[
    "anthropic",
    "openai",
    "google",
    "mistral",
    "ollama",
    "openrouter",
    "xai",
];

impl Config {
    fn get_provider(&self, name: &str) -> Option<&ProviderConfig> {
        match name {
            "anthropic" => self.anthropic.as_ref(),
            "openai" => self.openai.as_ref(),
            "google" => self.google.as_ref(),
            "mistral" => self.mistral.as_ref(),
            "ollama" => self.ollama.as_ref(),
            "openrouter" => self.openrouter.as_ref(),
            "xai" => self.xai.as_ref(),
            _ => None,
        }
    }

    /// Returns all configured providers in fixed order.
    pub fn available_providers(&self) -> Vec<(String, ProviderConfig)> {
        PROVIDER_ORDER
            .iter()
            .filter_map(|&name| {
                self.get_provider(name)
                    .map(|cfg| (name.to_string(), cfg.clone()))
            })
            .collect()
    }
}

/// Update the `provider = "..."` line in config.toml without touching anything else.
pub fn save_default_provider(name: &str) {
    let path = config_path();
    let Ok(content) = fs::read_to_string(&path) else {
        return;
    };
    let updated = content
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("provider") && line.contains('=') {
                // Preserve any inline comment.
                let comment = line.find('#').map(|i| &line[i..]).unwrap_or("");
                if comment.is_empty() {
                    format!("provider = \"{name}\"")
                } else {
                    format!("provider = \"{name}\"  {comment}")
                }
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let _ = fs::write(&path, updated);
}
