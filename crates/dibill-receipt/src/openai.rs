//! Request and response shapes for OpenAI-compatible chat completions with an image, as served
//! by `OpenAI`, `OpenRouter`, Google's `OpenAI` endpoint for Gemini, Mistral and local servers (vLLM,
//! Ollama, llama.cpp). Pure functions: the HTTP lives in the `http` feature.

use serde_json::{Value, json};

use crate::model::{ModelReceipt, json_schema};
use crate::normalize::Issue;
use crate::prompt::SYSTEM_PROMPT;

/// How the provider is asked for JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    /// `response_format: json_schema` with `strict: true`: the provider enforces the schema.
    #[default]
    JsonSchema,
    /// `response_format: json_object`: valid JSON only; the schema goes in the prompt.
    JsonObject,
    /// No `response_format` (providers that support neither); JSON is requested in the prompt.
    Prompt,
}

impl OutputMode {
    /// `json_schema`, `json_object` or `prompt`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "json_schema" => Some(Self::JsonSchema),
            "json_object" => Some(Self::JsonObject),
            "prompt" => Some(Self::Prompt),
            _ => None,
        }
    }
}

/// An earlier answer and what was wrong with it, for the repair retry.
#[derive(Debug, Clone)]
pub struct Repair<'a> {
    pub previous: &'a str,
    pub problems: &'a [String],
}

/// The request body for one receipt image (`data:` URL or `https:` URL).
#[must_use]
pub fn request_body(
    model: &str,
    mode: OutputMode,
    image_url: &str,
    repair: Option<&Repair<'_>>,
) -> Value {
    let mut system = SYSTEM_PROMPT.to_owned();
    if mode != OutputMode::JsonSchema {
        system.push_str("\n\nAnswer with a single JSON object (no prose, no code fences) matching this JSON Schema:\n");
        system.push_str(&json_schema().to_string());
    }
    let mut messages = vec![
        json!({ "role": "system", "content": system }),
        json!({ "role": "user", "content": [
            { "type": "text", "text": "Read this receipt." },
            { "type": "image_url", "image_url": { "url": image_url } }
        ]}),
    ];
    if let Some(repair) = repair {
        messages.push(json!({ "role": "assistant", "content": repair.previous }));
        messages.push(json!({ "role": "user", "content": format!(
            "Your answer has these problems:\n- {}\nLook at the receipt again and answer with the corrected JSON.",
            repair.problems.join("\n- ")
        )}));
    }
    let mut body = json!({ "model": model, "messages": messages });
    match mode {
        OutputMode::JsonSchema => {
            body["response_format"] = json!({
                "type": "json_schema",
                "json_schema": { "name": "receipt", "strict": true, "schema": json_schema() }
            });
        }
        OutputMode::JsonObject => body["response_format"] = json!({ "type": "json_object" }),
        OutputMode::Prompt => {}
    }
    body
}

/// Token counts and, when the provider reports it (`OpenRouter`), the cost in USD of each request
/// as printed (decimal text, summed by the harness without floats).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: Vec<String>,
}

impl Usage {
    pub fn add(&mut self, other: &Self) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cost_usd.extend(other.cost_usd.iter().cloned());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResponseError {
    #[error("the response isn't a chat completion: {0}")]
    Shape(String),
    #[error("the model refused: {0}")]
    Refused(String),
}

/// The message text and usage from a chat-completions response body.
///
/// # Errors
/// [`ResponseError`] when there is no message content.
pub fn parse_response(body: &str) -> Result<(String, Usage), ResponseError> {
    let value: Value =
        serde_json::from_str(body).map_err(|e| ResponseError::Shape(e.to_string()))?;
    let message = &value["choices"][0]["message"];
    if let Some(refusal) = message["refusal"].as_str().filter(|r| !r.is_empty()) {
        return Err(ResponseError::Refused(refusal.to_owned()));
    }
    let content = message["content"]
        .as_str()
        .ok_or_else(|| ResponseError::Shape(truncate(body, 300)))?
        .to_owned();
    let usage = &value["usage"];
    Ok((
        content,
        Usage {
            input_tokens: usage["prompt_tokens"].as_u64().unwrap_or(0),
            output_tokens: usage["completion_tokens"].as_u64().unwrap_or(0),
            cost_usd: usage
                .get("cost")
                .filter(|c| c.is_number())
                .map(Value::to_string)
                .into_iter()
                .collect(),
        },
    ))
}

/// The receipt in a message, tolerating code fences and text around the JSON object.
///
/// # Errors
/// The JSON error when no receipt object can be read.
pub fn parse_receipt(content: &str) -> Result<ModelReceipt, serde_json::Error> {
    let trimmed = content.trim();
    serde_json::from_str(trimmed).or_else(|e| match (trimmed.find('{'), trimmed.rfind('}')) {
        (Some(a), Some(b)) if a < b => serde_json::from_str(&trimmed[a..=b]),
        _ => Err(e),
    })
}

/// Problems with an answer, in words for the model.
#[must_use]
pub fn describe(issues: &[Issue]) -> Vec<String> {
    issues.iter().map(ToString::to_string).collect()
}

fn truncate(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &text[..i]),
        None => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_schema_request() {
        let body = request_body(
            "m",
            OutputMode::JsonSchema,
            "data:image/png;base64,AA",
            None,
        );
        assert_eq!(body["model"], "m");
        assert_eq!(body["response_format"]["json_schema"]["strict"], true);
        assert_eq!(body["messages"].as_array().map(Vec::len), Some(2));
        assert_eq!(
            body["messages"][1]["content"][1]["image_url"]["url"],
            "data:image/png;base64,AA"
        );
        assert!(
            !body["messages"][0]["content"]
                .as_str()
                .unwrap_or("")
                .contains("JSON Schema:")
        );
    }

    #[test]
    fn other_modes_put_the_schema_in_the_prompt() {
        let body = request_body("m", OutputMode::JsonObject, "u", None);
        assert_eq!(body["response_format"]["type"], "json_object");
        assert!(
            body["messages"][0]["content"]
                .as_str()
                .unwrap_or("")
                .contains("JSON Schema:")
        );
        let body = request_body("m", OutputMode::Prompt, "u", None);
        assert!(body.get("response_format").is_none());
    }

    #[test]
    fn repair_appends_the_conversation() {
        let problems = vec!["the item totals add up to 3.00 but the total is 3.50".to_owned()];
        let repair = Repair {
            previous: "{}",
            problems: &problems,
        };
        let body = request_body("m", OutputMode::JsonSchema, "u", Some(&repair));
        let messages = body["messages"].as_array().expect("array");
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[2]["role"], "assistant");
        assert!(
            messages[3]["content"]
                .as_str()
                .unwrap_or("")
                .contains("3.50")
        );
    }

    #[test]
    fn reads_responses() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"```json\n{\"merchant\":null,\"date\":null,\"currency\":\"EUR\",\"items\":[{\"description\":\"A\",\"quantity\":null,\"unit_price\":null,\"total\":\"1.00\"}],\"total\":\"1.00\"}\n```"}}],
            "usage":{"prompt_tokens":1200,"completion_tokens":80,"cost":0.00042}}"#;
        let (content, usage) = parse_response(body).expect("parses");
        assert_eq!(usage.input_tokens, 1200);
        assert_eq!(usage.cost_usd, ["0.00042"]);
        let receipt = parse_receipt(&content).expect("receipt");
        assert_eq!(receipt.items[0].total, "1.00");

        assert!(matches!(
            parse_response(r#"{"choices":[{"message":{"content":null,"refusal":"no"}}]}"#),
            Err(ResponseError::Refused(_))
        ));
        assert!(parse_response(r#"{"error":{"message":"bad"}}"#).is_err());
    }

    #[test]
    fn sums_usage() {
        let mut a = Usage {
            input_tokens: 1,
            output_tokens: 2,
            cost_usd: vec!["0.1".into()],
        };
        a.add(&Usage {
            input_tokens: 3,
            output_tokens: 4,
            cost_usd: vec!["0.2".into()],
        });
        assert_eq!((a.input_tokens, a.output_tokens), (4, 6));
        assert_eq!(a.cost_usd, ["0.1", "0.2"]);
    }
}
