//! Receipt scanning (ADR 0008).
//!
//! A vision model reads a receipt photo and answers with a [`ModelReceipt`]: amounts as decimal
//! strings, exactly as printed, constrained by [`json_schema`]. Everything after that runs here,
//! without floats: [`check`] finds what is wrong with an answer (and drives one repair retry), and
//! [`normalize`] turns it into minor units with discounts folded into their items.
//!
//! The provider is any OpenAI-compatible chat-completions endpoint ([`openai`]) or Claude's
//! Messages API ([`anthropic`]); the HTTP client
//! (feature `http`) is used by the server and the `receipt-eval` harness, never by the apps.

mod amount;
pub mod anthropic;
#[cfg(feature = "http")]
mod client;
pub mod eval;
mod model;
mod normalize;
pub mod openai;
mod prompt;

pub use amount::{AmountError, parse_amount, parse_date};
#[cfg(feature = "http")]
pub use client::{Api, ProviderConfig, ScanError, ScanOutcome, Scanner};
pub use model::{ModelItem, ModelReceipt, json_schema};
pub use normalize::{Issue, Receipt, ReceiptItem, check, normalize};
pub use prompt::SYSTEM_PROMPT;
