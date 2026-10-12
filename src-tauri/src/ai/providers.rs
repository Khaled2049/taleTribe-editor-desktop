//! Talking to the three providers. Building a request and reading a streamed
//! line are pure functions so they can be tested without a network; `stream`
//! and `list_models` are the only parts that do I/O.

use futures_util::StreamExt;
use serde::Serialize;
use serde_json::{json, Value};

use super::prompts::Prompt;
use super::settings::ModelChoice;
use super::{AiError, Provider};
use crate::secrets::Secret;

const ANTHROPIC_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const OPENAI_URL: &str = "https://api.openai.com";

/// Anthropic counts reasoning inside `max_tokens`, so the ceiling is set well
/// above the answer size; the prompt is what keeps the answer short.
const ANTHROPIC_MAX_TOKENS: u32 = 16_000;
const OPENAI_MAX_TOKENS: u32 = 8_192;

/// Models whose safety classifiers can decline a request and that accept a
/// server-side fallback, which re-runs a declined request on another model.
const FALLBACK_MODELS: [&str; 4] = [
    "claude-fable-5-1",
    "claude-opus-5-5",
    "claude-opus-5",
    "claude-sonnet-5-5",
];
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

/// Everything needed to reach one model.
#[derive(Debug)]
pub struct Target {
    pub provider: Provider,
    pub ollama_url: String,
    pub model: ModelChoice,
    pub key: Option<Secret>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    pub supports_effort: bool,
    pub can_think: bool,
}

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        // A local model can take minutes to load before its first token.
        .read_timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("static client configuration is valid")
}

// ── Requests ─────────────────────────────────────────────────────────────────

fn uses_fallbacks(model: &str) -> bool {
    FALLBACK_MODELS.contains(&model)
}

fn anthropic_body(model: &ModelChoice, prompt: &Prompt) -> Value {
    let mut body = json!({
        "model": model.id,
        "max_tokens": ANTHROPIC_MAX_TOKENS,
        "stream": true,
        "system": prompt.system,
        "messages": [{ "role": "user", "content": prompt.user }],
    });
    if model.supports_effort {
        body["output_config"] = json!({ "effort": prompt.effort.as_str() });
    }
    if uses_fallbacks(&model.id) {
        body["fallbacks"] = json!("default");
    }
    body
}

fn openai_body(model: &ModelChoice, prompt: &Prompt) -> Value {
    json!({
        "model": model.id,
        "stream": true,
        "max_completion_tokens": OPENAI_MAX_TOKENS,
        "messages": [
            { "role": "system", "content": prompt.system },
            { "role": "user", "content": prompt.user },
        ],
    })
}

fn ollama_body(model: &ModelChoice, prompt: &Prompt) -> Value {
    // Ollama silently drops the start of a prompt longer than its context
    // window, so the window is sized to this prompt (about 3 characters a token).
    let needed = (prompt.system.len() + prompt.user.len()) / 3 + prompt.answer_tokens as usize * 2;
    let context = needed.div_ceil(1024).max(4) * 1024;
    let mut body = json!({
        "model": model.id,
        "stream": true,
        "messages": [
            { "role": "system", "content": prompt.system },
            { "role": "user", "content": prompt.user },
        ],
        "options": { "num_predict": prompt.answer_tokens * 2, "num_ctx": context },
    });
    if model.can_think {
        // Reasoning is never shown here and makes a suggestion slow to arrive.
        body["think"] = json!(false);
    }
    body
}

fn request(
    client: &reqwest::Client,
    target: &Target,
    prompt: &Prompt,
) -> Result<reqwest::RequestBuilder, AiError> {
    let key = || target.key.as_ref().ok_or(AiError::MissingKey);
    Ok(match target.provider {
        Provider::Ollama => client
            .post(format!("{}/api/chat", target.ollama_url))
            .json(&ollama_body(&target.model, prompt)),
        Provider::Anthropic => {
            let mut request = client
                .post(format!("{ANTHROPIC_URL}/v1/messages"))
                .header("x-api-key", key()?.expose())
                .header("anthropic-version", ANTHROPIC_VERSION)
                .json(&anthropic_body(&target.model, prompt));
            if uses_fallbacks(&target.model.id) {
                request = request.header("anthropic-beta", FALLBACK_BETA);
            }
            request
        }
        Provider::Openai => client
            .post(format!("{OPENAI_URL}/v1/chat/completions"))
            .bearer_auth(key()?.expose())
            .json(&openai_body(&target.model, prompt)),
    })
}

// ── Reading a stream ─────────────────────────────────────────────────────────

/// What one line of a streamed response contributed.
#[derive(Debug, Default, PartialEq)]
struct Step {
    text: Option<String>,
    done: bool,
}

fn text(value: &Value) -> Option<String> {
    value.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

/// Server-sent events: only `data:` lines carry JSON.
fn sse_data(line: &str) -> Option<&str> {
    line.strip_prefix("data:").map(str::trim)
}

fn parse_anthropic(line: &str) -> Result<Step, AiError> {
    let Some(event) = sse_data(line).and_then(|data| serde_json::from_str::<Value>(data).ok())
    else {
        return Ok(Step::default());
    };
    match event["type"].as_str() {
        // Thinking and fallback blocks arrive as other delta types and are skipped.
        Some("content_block_delta") if event["delta"]["type"] == "text_delta" => Ok(Step {
            text: text(&event["delta"]["text"]),
            done: false,
        }),
        Some("message_delta") if event["delta"]["stop_reason"] == "refusal" => {
            Err(AiError::Refused)
        }
        Some("message_stop") => Ok(Step {
            text: None,
            done: true,
        }),
        Some("error") => Err(provider_error(&event)),
        _ => Ok(Step::default()),
    }
}

fn parse_openai(line: &str) -> Result<Step, AiError> {
    let Some(data) = sse_data(line) else {
        return Ok(Step::default());
    };
    if data == "[DONE]" {
        return Ok(Step {
            text: None,
            done: true,
        });
    }
    let Ok(event) = serde_json::from_str::<Value>(data) else {
        return Ok(Step::default());
    };
    if !event["error"].is_null() {
        return Err(provider_error(&event));
    }
    let choice = &event["choices"][0];
    if choice["delta"]["refusal"].is_string() || choice["finish_reason"] == "content_filter" {
        return Err(AiError::Refused);
    }
    Ok(Step {
        text: text(&choice["delta"]["content"]),
        done: false,
    })
}

fn parse_ollama(line: &str) -> Result<Step, AiError> {
    let Ok(event) = serde_json::from_str::<Value>(line) else {
        return Ok(Step::default());
    };
    if !event["error"].is_null() {
        return Err(provider_error(&event));
    }
    Ok(Step {
        text: text(&event["message"]["content"]),
        done: event["done"] == true,
    })
}

/// The provider's own message, shortened. Providers put it at `error.message`
/// or directly at `error`.
fn provider_error(body: &Value) -> AiError {
    let error = &body["error"];
    let message = error["message"]
        .as_str()
        .or(error.as_str())
        .unwrap_or("unknown error");
    AiError::Provider(message.chars().take(200).collect())
}

fn status_error(target: &Target, status: u16, body: &str) -> AiError {
    match status {
        401 | 403 => AiError::BadKey,
        404 => AiError::ModelMissing(target.model.id.clone()),
        429 | 529 => AiError::RateLimited,
        _ => match serde_json::from_str::<Value>(body) {
            Ok(json) if !json["error"].is_null() => provider_error(&json),
            _ => AiError::Provider(format!("HTTP {status}")),
        },
    }
}

fn transport_error(target: &Target, error: &reqwest::Error) -> AiError {
    if target.provider == Provider::Ollama && (error.is_connect() || error.is_timeout()) {
        AiError::OllamaUnreachable(target.ollama_url.clone())
    } else {
        AiError::Network
    }
}

/// Collects bytes and hands back whole lines, so a line or a multi-byte
/// character split across two network chunks is never read half-way.
#[derive(Default)]
struct Lines(Vec<u8>);

impl Lines {
    fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.0.extend_from_slice(bytes);
        let mut lines = Vec::new();
        while let Some(end) = self.0.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.0.drain(..=end).collect();
            lines.push(String::from_utf8_lossy(&line).trim_end().to_string());
        }
        lines
    }
}

/// Sends the prompt and calls `on_text` with each piece of the answer as it
/// arrives. Returns the whole answer.
pub async fn stream(
    client: &reqwest::Client,
    target: &Target,
    prompt: &Prompt,
    mut on_text: impl FnMut(&str),
) -> Result<String, AiError> {
    if target.model.id.is_empty() {
        return Err(AiError::NotConfigured);
    }
    let response = request(client, target, prompt)?
        .send()
        .await
        .map_err(|e| transport_error(target, &e))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(status_error(target, status.as_u16(), &body));
    }

    let parse = match target.provider {
        Provider::Ollama => parse_ollama,
        Provider::Anthropic => parse_anthropic,
        Provider::Openai => parse_openai,
    };
    let mut answer = String::new();
    let mut lines = Lines::default();
    let mut body = response.bytes_stream();
    'read: while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|e| transport_error(target, &e))?;
        for line in lines.push(&chunk) {
            let step = parse(&line)?;
            if let Some(text) = step.text {
                on_text(&text);
                answer.push_str(&text);
            }
            if step.done {
                break 'read;
            }
        }
    }
    if answer.trim().is_empty() {
        return Err(AiError::BadAnswer);
    }
    Ok(answer)
}

// ── Listing models ───────────────────────────────────────────────────────────

fn models_from(provider: Provider, body: &Value) -> Vec<ModelInfo> {
    let field = if provider == Provider::Ollama {
        "models"
    } else {
        "data"
    };
    let listed = body[field]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut models: Vec<ModelInfo> = listed
        .iter()
        .filter_map(|model| {
            let id = model[if provider == Provider::Ollama {
                "name"
            } else {
                "id"
            }]
            .as_str()?;
            Some(ModelInfo {
                id: id.to_string(),
                label: model["display_name"].as_str().unwrap_or(id).to_string(),
                supports_effort: model["capabilities"]["effort"]["supported"] == true,
                can_think: model["capabilities"]
                    .as_array()
                    .is_some_and(|caps| caps.iter().any(|c| c == "thinking")),
            })
        })
        .collect();
    if provider == Provider::Openai {
        // The list mixes in models that cannot hold a conversation.
        const NOT_CHAT: [&str; 9] = [
            "embedding",
            "whisper",
            "tts",
            "dall-e",
            "image",
            "audio",
            "moderation",
            "realtime",
            "transcribe",
        ];
        models.retain(|m| !NOT_CHAT.iter().any(|word| m.id.contains(word)));
        models.sort_by(|a, b| a.id.cmp(&b.id));
    }
    models
}

pub async fn list_models(
    client: &reqwest::Client,
    target: &Target,
) -> Result<Vec<ModelInfo>, AiError> {
    let key = || target.key.as_ref().ok_or(AiError::MissingKey);
    let request = match target.provider {
        Provider::Ollama => client.get(format!("{}/api/tags", target.ollama_url)),
        Provider::Anthropic => client
            .get(format!("{ANTHROPIC_URL}/v1/models?limit=1000"))
            .header("x-api-key", key()?.expose())
            .header("anthropic-version", ANTHROPIC_VERSION),
        Provider::Openai => client
            .get(format!("{OPENAI_URL}/v1/models"))
            .bearer_auth(key()?.expose()),
    };
    let response = request
        .send()
        .await
        .map_err(|e| transport_error(target, &e))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| transport_error(target, &e))?;
    if !status.is_success() {
        return Err(status_error(target, status.as_u16(), &body));
    }
    let json = serde_json::from_str(&body).map_err(|_| AiError::BadAnswer)?;
    Ok(models_from(target.provider, &json))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::prompts::Effort;

    fn prompt() -> Prompt {
        Prompt {
            system: "Be brief.".into(),
            user: "x".repeat(30_000),
            answer_tokens: 512,
            effort: Effort::Low,
        }
    }

    fn model(id: &str) -> ModelChoice {
        ModelChoice {
            id: id.into(),
            supports_effort: true,
            can_think: true,
        }
    }

    #[test]
    fn anthropic_requests_carry_effort_and_a_fallback_only_where_supported() {
        let current = anthropic_body(&model("claude-opus-5-5"), &prompt());
        assert_eq!(current["output_config"]["effort"], "low");
        assert_eq!(current["fallbacks"], "default");
        assert_eq!(current["max_tokens"], ANTHROPIC_MAX_TOKENS);
        assert!(current.get("thinking").is_none());

        let older = ModelChoice {
            id: "claude-haiku-4-5".into(),
            ..ModelChoice::default()
        };
        let body = anthropic_body(&older, &prompt());
        assert!(body.get("output_config").is_none());
        assert!(body.get("fallbacks").is_none());
    }

    #[test]
    fn ollama_requests_size_the_context_window_to_the_prompt() {
        let body = ollama_body(&model("qwen3.5:9b"), &prompt());
        let window = body["options"]["num_ctx"].as_u64().unwrap();
        assert!(window >= 30_000 / 3 + 1024, "window {window}");
        assert_eq!(window % 1024, 0);
        assert_eq!(body["think"], false);

        let plain = ModelChoice {
            id: "llama3".into(),
            ..ModelChoice::default()
        };
        assert!(ollama_body(&plain, &prompt()).get("think").is_none());
    }

    #[test]
    fn each_provider_stream_format_is_read() {
        let said = |text: &str| {
            Ok(Step {
                text: Some(text.into()),
                done: false,
            })
        };
        let finished = Ok(Step {
            text: None,
            done: true,
        });

        assert_eq!(
            parse_anthropic(
                r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}"#
            ),
            said("Hi")
        );
        assert_eq!(
            parse_anthropic("event: content_block_delta"),
            Ok(Step::default())
        );
        assert_eq!(
            parse_anthropic(
                r#"data: {"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":""}}"#
            ),
            Ok(Step::default())
        );
        assert_eq!(
            parse_anthropic(r#"data: {"type":"message_stop"}"#),
            finished
        );
        assert!(matches!(
            parse_anthropic(r#"data: {"type":"message_delta","delta":{"stop_reason":"refusal"}}"#),
            Err(AiError::Refused)
        ));
        assert!(matches!(
            parse_anthropic(r#"data: {"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#),
            Err(AiError::Provider(m)) if m == "Overloaded"
        ));

        assert_eq!(
            parse_openai(r#"data: {"choices":[{"delta":{"content":"Hi"}}]}"#),
            said("Hi")
        );
        assert_eq!(
            parse_openai(r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#),
            Ok(Step::default())
        );
        assert_eq!(parse_openai("data: [DONE]"), finished);

        assert_eq!(
            parse_ollama(r#"{"message":{"role":"assistant","content":"Hi"},"done":false}"#),
            said("Hi")
        );
        assert_eq!(
            parse_ollama(r#"{"message":{"content":""},"done":true}"#),
            finished
        );
        assert!(matches!(
            parse_ollama(r#"{"error":"model 'x' not found"}"#),
            Err(AiError::Provider(m)) if m.contains("not found")
        ));
    }

    #[test]
    fn lines_survive_being_split_across_network_chunks() {
        let mut lines = Lines::default();
        let bytes = "data: caf\u{e9}\r\nsecond\n".as_bytes();
        // Cut in the middle of the two-byte "é".
        let cut = bytes.iter().position(|&b| b == 0xC3).unwrap() + 1;
        assert!(lines.push(&bytes[..cut]).is_empty());
        assert_eq!(lines.push(&bytes[cut..]), ["data: caf\u{e9}", "second"]);
    }

    #[test]
    fn http_failures_become_errors_the_writer_can_act_on() {
        let target = Target {
            provider: Provider::Ollama,
            ollama_url: "http://localhost:11434".into(),
            model: model("nope"),
            key: None,
        };
        assert!(matches!(status_error(&target, 401, ""), AiError::BadKey));
        assert!(matches!(status_error(&target, 404, ""), AiError::ModelMissing(m) if m == "nope"));
        assert!(matches!(
            status_error(&target, 429, ""),
            AiError::RateLimited
        ));
        assert!(matches!(
            status_error(&target, 400, r#"{"error":{"message":"bad field"}}"#),
            AiError::Provider(m) if m == "bad field"
        ));
        assert!(
            matches!(status_error(&target, 500, "<html>"), AiError::Provider(m) if m == "HTTP 500")
        );
    }

    #[test]
    fn model_lists_are_read_from_each_provider_shape() {
        let ollama =
            json!({"models":[{"name":"qwen3.5:9b","capabilities":["completion","thinking"]}]});
        let listed = models_from(Provider::Ollama, &ollama);
        assert_eq!(listed[0].id, "qwen3.5:9b");
        assert!(listed[0].can_think);

        let anthropic = json!({"data":[{"id":"claude-opus-5-5","display_name":"Claude Opus 5.5",
            "capabilities":{"effort":{"supported":true}}}]});
        let listed = models_from(Provider::Anthropic, &anthropic);
        assert_eq!(listed[0].label, "Claude Opus 5.5");
        assert!(listed[0].supports_effort && !listed[0].can_think);

        let openai = json!({"data":[{"id":"text-embedding-3-small"},{"id":"gpt-x"}]});
        let listed = models_from(Provider::Openai, &openai);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "gpt-x");
    }
}
