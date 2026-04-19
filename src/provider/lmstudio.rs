use super::ApiEvent;
use crate::config::ProviderConfig;
use tokio::sync::mpsc;

pub async fn stream(
    cfg: &ProviderConfig,
    query: &str,
    system_prompt: &str,
    tx: mpsc::UnboundedSender<ApiEvent>,
) {
    let client = reqwest::Client::new();
    let base_url = if cfg.api_url.is_empty() {
        "http://localhost:1234"
    } else {
        cfg.api_url.trim_end_matches('/')
    };
    let url = format!("{base_url}/api/v1/chat");

    let body = serde_json::json!({
        "model": cfg.model,
        "system_prompt": system_prompt,
        "input": query,
    });

    let request = client
        .post(&url)
        .header("content-type", "application/json")
        .json(&body);

    let Some(resp) = super::send_request(request, &tx).await else {
        return;
    };

    let json = match resp.json::<serde_json::Value>().await {
        Ok(j) => j,
        Err(e) => {
            let _ = tx.send(ApiEvent::Error(e.to_string()));
            return;
        }
    };

    let outputs = json["output"].as_array();
    let has_content = outputs
        .map(|arr| arr.iter().any(|item| item["content"].as_str().is_some()))
        .unwrap_or(false);

    if !has_content {
        let err_msg = json["error"]
            .as_str()
            .unwrap_or("invalid response: output[].content missing");
        let _ = tx.send(ApiEvent::Error(err_msg.to_string()));
        return;
    }

    for item in outputs.unwrap() {
        if let Some(text) = item["content"].as_str() {
            if !text.is_empty() && tx.send(ApiEvent::Delta(text.to_string())).is_err() {
                return;
            }
        }
    }

    let _ = tx.send(ApiEvent::Done);
}
