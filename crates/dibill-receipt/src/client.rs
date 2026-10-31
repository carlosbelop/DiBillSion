use std::sync::Once;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use dibill_domain::Currency;

use crate::anthropic;
use crate::model::ModelReceipt;
use crate::normalize::check;
use crate::openai::{self, OutputMode, Repair, Usage, describe, parse_receipt};

/// Which protocol the provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Api {
    /// `POST {base}/chat/completions`, bearer key, asking for JSON the given way.
    OpenAi(OutputMode),
    /// Claude's Messages API: `POST {base}/messages`, `x-api-key`, structured output.
    Anthropic,
}

impl Api {
    /// `openai` (with `mode`) or `anthropic`.
    #[must_use]
    pub fn parse(name: &str, mode: OutputMode) -> Option<Self> {
        match name {
            "openai" => Some(Self::OpenAi(mode)),
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }
}

/// Which provider and model to call.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub api: Api,
    /// Up to and including the version, e.g. `https://openrouter.ai/api/v1` or
    /// [`anthropic::BASE_URL`]; the protocol's path is appended.
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    /// Per request.
    pub timeout: Duration,
}

/// The result of one scan, valid or not, with everything the harness measures.
#[derive(Debug, Clone)]
pub struct ScanOutcome {
    /// The best answer: the repaired one when the retry fixed something.
    pub receipt: Option<ModelReceipt>,
    /// Every raw answer, in order.
    pub raw: Vec<String>,
    pub usage: Usage,
    pub elapsed: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("provider unreachable: {0}")]
    Network(String),
    #[error("provider error {status}: {body}")]
    Provider { status: u16, body: String },
    #[error(transparent)]
    Response(#[from] crate::openai::ResponseError),
}

/// Calls an OpenAI-compatible endpoint. Cheap to clone; reuse one per process.
#[derive(Debug, Clone)]
pub struct Scanner {
    http: reqwest::Client,
    config: ProviderConfig,
}

impl Scanner {
    /// # Errors
    /// [`ScanError::Network`] if the HTTP client can't be built (no system certificates).
    pub fn new(config: ProviderConfig) -> Result<Self, ScanError> {
        static PROVIDER: Once = Once::new();
        PROVIDER.call_once(|| {
            // Fails only if another provider was installed first, which is fine.
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| ScanError::Network(e.to_string()))?;
        Ok(Self { http, config })
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.config.model
    }

    /// Reads one receipt image (`mime` such as `image/jpeg`). When the answer can't be parsed or
    /// its items don't add up to its total, asks once more with the problems spelled out
    /// (amounts read in `fallback` unless the receipt names its currency).
    ///
    /// # Errors
    /// [`ScanError`] when the provider can't be reached or answers with an error. An answer that
    /// stays invalid after the retry is not an error: [`ScanOutcome::receipt`] is `None` or
    /// [`check`] still reports issues.
    pub async fn scan(
        &self,
        image: &[u8],
        mime: &str,
        fallback: Currency,
    ) -> Result<ScanOutcome, ScanError> {
        let started = Instant::now();
        let encoded = STANDARD.encode(image);
        let mut usage = Usage::default();
        let mut raw = Vec::new();

        let (first, first_usage) = self.complete(&encoded, mime, None).await?;
        usage.add(&first_usage);
        raw.push(first.clone());
        let parsed = parse_receipt(&first);
        let problems = match &parsed {
            Ok(receipt) => describe(&check(receipt, fallback)),
            Err(e) => vec![format!("the answer is not valid JSON for the schema: {e}")],
        };
        if problems.is_empty() {
            return Ok(ScanOutcome {
                receipt: parsed.ok(),
                raw,
                usage,
                elapsed: started.elapsed(),
            });
        }

        let repair = Repair {
            previous: &first,
            problems: &problems,
        };
        let (second, second_usage) = self.complete(&encoded, mime, Some(&repair)).await?;
        usage.add(&second_usage);
        raw.push(second.clone());
        let receipt = match (parsed, parse_receipt(&second)) {
            (Ok(first), Ok(second)) => {
                if check(&second, fallback).len() <= check(&first, fallback).len() {
                    second
                } else {
                    first
                }
            }
            (Err(_), Ok(r)) | (Ok(r), Err(_)) => r,
            (Err(_), Err(_)) => {
                return Ok(ScanOutcome {
                    receipt: None,
                    raw,
                    usage,
                    elapsed: started.elapsed(),
                });
            }
        };
        Ok(ScanOutcome {
            receipt: Some(receipt),
            raw,
            usage,
            elapsed: started.elapsed(),
        })
    }

    async fn complete(
        &self,
        image_base64: &str,
        mime: &str,
        repair: Option<&Repair<'_>>,
    ) -> Result<(String, Usage), ScanError> {
        let base = self.config.base_url.trim_end_matches('/');
        let request = match self.config.api {
            Api::OpenAi(mode) => {
                let url = format!("data:{mime};base64,{image_base64}");
                self.http
                    .post(format!("{base}/chat/completions"))
                    .bearer_auth(&self.config.api_key)
                    .body(openai::request_body(&self.config.model, mode, &url, repair).to_string())
            }
            Api::Anthropic => self
                .http
                .post(format!("{base}/messages"))
                .header("x-api-key", &self.config.api_key)
                .header("anthropic-version", anthropic::API_VERSION)
                .body(
                    anthropic::request_body(&self.config.model, image_base64, mime, repair)
                        .to_string(),
                ),
        };
        let response = request
            .header("content-type", "application/json")
            .send()
            .await
            .map_err(|e| ScanError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .map_err(|e| ScanError::Network(e.to_string()))?;
        if !(200..300).contains(&status) {
            return Err(ScanError::Provider {
                status,
                body: text.chars().take(500).collect(),
            });
        }
        Ok(match self.config.api {
            Api::OpenAi(_) => openai::parse_response(&text)?,
            Api::Anthropic => anthropic::parse_response(&text)?,
        })
    }
}
