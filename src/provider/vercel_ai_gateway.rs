use super::ApiEvent;
use crate::config::ProviderConfig;
use tokio::sync::mpsc;

const DEFAULT_BASE_URL: &str = "https://ai-gateway.vercel.sh/v1";

pub async fn stream(
    cfg: &ProviderConfig,
    query: &str,
    system_prompt: &str,
    tx: mpsc::UnboundedSender<ApiEvent>,
) {
    let base_url = if cfg.api_url.is_empty() {
        DEFAULT_BASE_URL
    } else {
        cfg.api_url.trim_end_matches('/')
    };

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": cfg.model,
        "instructions": system_prompt,
        "input": query,
        "stream": true,
        "max_output_tokens": 256,
    });

    let request = client
        .post(format!("{base_url}/responses"))
        .header("Authorization", format!("Bearer {}", cfg.api_key))
        .header("content-type", "application/json")
        .json(&body);

    let Some(resp) = super::send_request(request, &tx).await else {
        return;
    };

    super::parse_sse_stream(resp, &tx, |json| {
        if json["type"] == "response.output_text.delta" {
            json["delta"].as_str().map(String::from)
        } else {
            None
        }
    })
    .await;
}
