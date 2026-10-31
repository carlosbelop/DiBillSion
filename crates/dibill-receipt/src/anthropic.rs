//! Request and response shapes for Claude's Messages API (`POST {base}/messages`), the native
//! alternative to the OpenAI-compatible protocol in [`crate::openai`]. Pure functions: the HTTP
//! lives in the `http` feature.

use serde_json::{Value, json};

use crate::model::json_schema;
use crate::openai::{Repair, ResponseError, Usage};
use crate::prompt::SYSTEM_PROMPT;

/// The API version this module speaks (`anthropic-version` header).
pub const API_VERSION: &str = "2026-10-21";
/// Default base URL.
pub const BASE_URL: &str = "https://api.anthropic.com/v1";
/// Room for the receipt JSON; long restaurant bills stay far below it.
const MAX_TOKENS: u32 = 16_000;

/// The request body for one receipt image, with structured output constrained to the schema.
#[must_use]
pub fn request_body(
    model: &str,
    image_base64: &str,
    mime: &str,
    repair: Option<&Repair<'_>>,
) -> Value {
    let mut messages = vec![json!({ "role": "user", "content": [
        { "type": "image", "source": { "type": "base64", "media_type": mime, "data": image_base64 } },
        { "type": "text", "text": "Read this receipt." }
    ]})];
    if let Some(repair) = repair {
        messages.push(json!({ "role": "assistant", "content": repair.previous }));
        messages.push(json!({ "role": "user", "content": format!(
            "Your answer has these problems:\n- {}\nLook at the receipt again and answer with the corrected JSON.",
            repair.problems.join("\n- ")
        )}));
    }
    json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "system": SYSTEM_PROMPT,
        "messages": messages,
        "output_config": { "format": { "type": "json_schema", "schema": json_schema() } }
    })
}

/// The answer text and usage from a Messages API response body. Thinking blocks are skipped;
/// a refusal is an error.
///
/// # Errors
/// [`ResponseError`] when there is no text answer.
pub fn parse_response(body: &str) -> Result<(String, Usage), ResponseError> {
    let value: Value =
        serde_json::from_str(body).map_err(|e| ResponseError::Shape(e.to_string()))?;
    if value["stop_reason"] == "refusal" {
        let why = value["stop_details"]["category"]
            .as_str()
            .unwrap_or("unspecified");
        return Err(ResponseError::Refused(why.to_owned()));
    }
    let text: String = value["content"]
        .as_array()
        .ok_or_else(|| ResponseError::Shape(body.chars().take(300).collect()))?
        .iter()
        .filter(|block| block["type"] == "text")
        .filter_map(|block| block["text"].as_str())
        .collect();
    if text.is_empty() {
        return Err(ResponseError::Shape(body.chars().take(300).collect()));
    }
    let usage = &value["usage"];
    Ok((
        text,
        Usage {
            input_tokens: usage["input_tokens"].as_u64().unwrap_or(0),
            output_tokens: usage["output_tokens"].as_u64().unwrap_or(0),
            cost_usd: Vec::new(),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_shape() {
        let body = request_body("claude-opus-5-5", "AAAA", "image/jpeg", None);
        assert_eq!(body["model"], "claude-opus-5-5");
        assert_eq!(body["system"], SYSTEM_PROMPT);
        let image = &body["messages"][0]["content"][0];
        assert_eq!(image["source"]["media_type"], "image/jpeg");
        assert_eq!(image["source"]["data"], "AAAA");
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert!(body.get("temperature").is_none());

        let problems = vec!["the total is missing or invalid".to_owned()];
        let repair = Repair {
            previous: "{}",
            problems: &problems,
        };
        let body = request_body("m", "AAAA", "image/png", Some(&repair));
        let messages = body["messages"].as_array().expect("array");
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[2]["role"], "user");
    }

    #[test]
    fn reads_responses() {
        let body = r#"{"content":[{"type":"thinking","thinking":""},{"type":"text","text":"{\"items\":[]}"}],
            "stop_reason":"end_turn","usage":{"input_tokens":1500,"output_tokens":300}}"#;
        let (text, usage) = parse_response(body).expect("parses");
        assert_eq!(text, r#"{"items":[]}"#);
        assert_eq!((usage.input_tokens, usage.output_tokens), (1500, 300));

        let refused = r#"{"content":[],"stop_reason":"refusal","stop_details":{"type":"refusal","category":"cyber"}}"#;
        assert_eq!(
            parse_response(refused),
            Err(ResponseError::Refused("cyber".into()))
        );
        assert!(parse_response(r#"{"type":"error","error":{"message":"x"}}"#).is_err());
        assert!(parse_response(r#"{"content":[{"type":"thinking","thinking":""}]}"#).is_err());
    }
}
