//! `POST /v1/receipts/scan` (ADR 0008): the image goes to the configured OpenAI-compatible model
//! and the answer comes back after validation and at most one repair retry. The provider key
//! never leaves the server. Nothing about the receipt is stored, only one row per scan for the
//! limits.

use std::time::Duration;

use axum::Json;
use axum::extract::State;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use dibill_domain::Currency;
use dibill_proto::{MAX_RECEIPT_BYTES, ScanReceipt, ScannedReceipt};
use dibill_receipt::openai::OutputMode;
use dibill_receipt::{Api, ProviderConfig, Scanner};

use crate::auth::Device;
use crate::extract::Body;
use crate::{AppError, AppState};

/// Receipt scanning settings: disabled without a provider.
#[derive(Debug, Clone)]
pub struct Receipts {
    scanner: Option<Scanner>,
    /// Scans per device in any 24 hours.
    daily_limit: u32,
    /// Scans per calendar month for the whole server: the budget cap.
    monthly_limit: u32,
}

impl Receipts {
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            scanner: None,
            daily_limit: 0,
            monthly_limit: 0,
        }
    }

    /// # Errors
    /// The HTTP client can't be built.
    pub fn new(
        provider: ProviderConfig,
        daily_limit: u32,
        monthly_limit: u32,
    ) -> Result<Self, AppError> {
        let scanner = Scanner::new(provider).map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(Self {
            scanner: Some(scanner),
            daily_limit,
            monthly_limit,
        })
    }

    /// From `RECEIPT_API` (`openai`, the default, or `anthropic` for Claude's Messages API),
    /// `RECEIPT_API_BASE` (required for `openai`; Anthropic's by default), `RECEIPT_API_KEY` and
    /// `RECEIPT_MODEL` (scanning stays off without a key and a model), `RECEIPT_OUTPUT_MODE`
    /// (`openai` only: `json_schema`, `json_object` or `prompt`; default `json_schema`), `RECEIPT_DAILY_LIMIT` (per device, default 20), `RECEIPT_MONTHLY_LIMIT`
    /// (whole server, default 1000) and `RECEIPT_TIMEOUT_SECS` (per model request, default 60).
    ///
    /// # Errors
    /// A setting that is present but invalid.
    pub fn from_env() -> Result<Self, AppError> {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        let mode = match var("RECEIPT_OUTPUT_MODE") {
            None => OutputMode::JsonSchema,
            Some(m) => OutputMode::parse(m.trim()).ok_or_else(|| {
                AppError::Internal(
                    "RECEIPT_OUTPUT_MODE is json_schema, json_object or prompt".into(),
                )
            })?,
        };
        let api_name = var("RECEIPT_API").unwrap_or_else(|| "openai".into());
        let api = Api::parse(api_name.trim(), mode)
            .ok_or_else(|| AppError::Internal("RECEIPT_API is openai or anthropic".into()))?;
        let base_url = var("RECEIPT_API_BASE").or_else(|| {
            (api == Api::Anthropic).then(|| dibill_receipt::anthropic::BASE_URL.to_owned())
        });
        let (Some(base_url), Some(api_key), Some(model)) =
            (base_url, var("RECEIPT_API_KEY"), var("RECEIPT_MODEL"))
        else {
            return Ok(Self::disabled());
        };
        let number = |name: &str, default: u32| -> Result<u32, AppError> {
            var(name).map_or(Ok(default), |v| {
                v.trim()
                    .parse()
                    .map_err(|_| AppError::Internal(format!("{name} must be a whole number")))
            })
        };
        let provider = ProviderConfig {
            api,
            base_url,
            api_key,
            model,
            timeout: Duration::from_secs(number("RECEIPT_TIMEOUT_SECS", 60)?.into()),
        };
        Self::new(
            provider,
            number("RECEIPT_DAILY_LIMIT", 20)?,
            number("RECEIPT_MONTHLY_LIMIT", 1000)?,
        )
    }

    /// The model in use, if scanning is on (for the startup log).
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.scanner.as_ref().map(Scanner::model)
    }
}

pub async fn scan(
    State(s): State<AppState>,
    device: Device,
    Body(req): Body<ScanReceipt>,
) -> Result<Json<ScannedReceipt>, AppError> {
    let receipts = &s.receipts;
    let scanner = receipts
        .scanner
        .as_ref()
        .ok_or(AppError::ReceiptsDisabled)?;
    if !matches!(req.mime.as_str(), "image/jpeg" | "image/png" | "image/webp") {
        return Err(AppError::Invalid(
            "the image must be JPEG, PNG or WebP".into(),
        ));
    }
    let image = STANDARD
        .decode(req.image_base64.as_bytes())
        .map_err(|_| AppError::Invalid("image_base64 is not valid base64".into()))?;
    if image.is_empty() {
        return Err(AppError::Invalid("empty image".into()));
    }
    if image.len() > MAX_RECEIPT_BYTES {
        return Err(AppError::TooLarge(format!(
            "the image is over {MAX_RECEIPT_BYTES} bytes"
        )));
    }
    let currency = Currency::from_code(req.currency.trim())
        .map_err(|_| AppError::Invalid(format!("unknown currency {}", req.currency)))?;

    // Reserve the scan first, so failed or concurrent requests count too.
    let db = s.db.get().await?;
    let reserved = db
        .query_opt(
            "WITH day AS (SELECT count(*) AS n FROM receipt_scans
                          WHERE device_id = $1 AND created_at > now() - interval '24 hours'),
                  month AS (SELECT count(*) AS n FROM receipt_scans
                            WHERE created_at >= date_trunc('month', now()))
             INSERT INTO receipt_scans (device_id, model)
             SELECT $1, $2 FROM day, month WHERE day.n < $3 AND month.n < $4
             RETURNING id, (SELECT n FROM day)",
            &[
                &device.0,
                &scanner.model(),
                &i64::from(receipts.daily_limit),
                &i64::from(receipts.monthly_limit),
            ],
        )
        .await?;
    let Some(row) = reserved else {
        return Err(AppError::LimitReached(
            "no receipt scans left for now; try again later".into(),
        ));
    };
    let id: i64 = row.get(0);
    let used_before: i64 = row.get(1);
    drop(db);

    let outcome = scanner.scan(&image, &req.mime, currency).await;
    let (ok, requests, input, output, ms) = match &outcome {
        Ok(o) => (
            Some(o.receipt.is_some()),
            i16::try_from(o.raw.len()).ok(),
            i64::try_from(o.usage.input_tokens).ok(),
            i64::try_from(o.usage.output_tokens).ok(),
            i32::try_from(o.elapsed.as_millis()).ok(),
        ),
        Err(_) => (Some(false), None, None, None, None),
    };
    s.db.get()
        .await?
        .execute(
            "UPDATE receipt_scans SET ok = $2, requests = $3, input_tokens = $4, output_tokens = $5,
                                      elapsed_ms = $6 WHERE id = $1",
            &[&id, &ok, &requests, &input, &output, &ms],
        )
        .await?;

    let outcome = outcome.map_err(|e| {
        tracing::warn!(error = %e, "receipt provider failed");
        AppError::ProviderFailed
    })?;
    let receipt = outcome.receipt.ok_or(AppError::Unreadable)?;
    let used = u32::try_from(used_before + 1).unwrap_or(u32::MAX);
    Ok(Json(ScannedReceipt {
        receipt,
        scans_left_today: receipts.daily_limit.saturating_sub(used),
    }))
}
