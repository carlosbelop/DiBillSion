# ADR 0008: Receipt scanning through any OpenAI-compatible vision model

- Status: accepted
- Date: 2026-10-31

## Context

Receipt scanning (SPEC §1.3, flow 8) turns a photo into line items the user assigns per person.
Vision language models read receipts well, but providers, prices and model quality change every
few months, and the owner hasn't picked one yet. We need:

- one integration that can talk to many providers, so the choice is a configuration change;
- answers we can trust to the cent, with no floats anywhere;
- a way to compare models on our own receipts before paying for one in production;
- no provider key on the phones.

## Decision

- **Provider protocol: OpenAI-compatible chat completions** (`POST {base}/chat/completions`, image as
  a `data:` URL, bearer key). It is served by OpenAI, OpenRouter (one key, most vendors' models),
  Google's OpenAI endpoint for Gemini, Mistral, and local servers (vLLM, Ollama, llama.cpp). The
  provider is three settings: base URL, model, key. A native client for another API is added only
  if the evaluation shows a model worth it that this protocol can't reach.
- **Structured output.** The model fills a fixed JSON Schema (`dibill_receipt::json_schema`,
  checked against the Rust types by a test) with `response_format: json_schema, strict: true`.
  Providers without it get `json_object` or the schema in the prompt (`OutputMode`).
- **Amounts are decimal strings exactly as printed** (`"2.50"`, `"-0.08"`), never JSON numbers,
  and are parsed into `i64` minor units in Rust with integer arithmetic (`parse_amount`).
- **Shape of the answer:** one entry per printed line, discounts as negative lines, free items as
  zero, taxes already included in prices omitted, tax/tip/service added on top as lines, and the
  amount paid as `total`. So the invariant is simply *items add up to the total*.
- **Validation and one repair retry.** `check` finds unreadable amounts, a missing total or items
  that don't add up. If anything is wrong, the conversation is sent back once with the problems
  in words; the answer with fewer problems wins. A receipt that still doesn't add up is shown to
  the user with the difference highlighted rather than rejected.
- **Normalization** (`normalize`): each negative line is folded into the nearest earlier item it
  fits in; discounts that fit nowhere are spread over all items in proportion (largest remainder,
  `dibill_domain::allocate`). Items are never negative and the sum never changes (property test).
- **Where it runs.** The model is called by the server (key in its environment, per-device
  limits), never by the apps. The pure parts live in `dibill-receipt` so the server, the core and
  the harness share them; the HTTP client is behind the crate's `http` feature (reqwest + rustls
  with the `ring` provider and the system's root certificates) and isn't linked into the apps.
- **Evaluation.** `eval/receipts/` holds receipt photos with hand-written ground truth in the
  answer's own format. `receipt-eval` runs any number of models over it through the same client
  and retry as production and reports parse rate, exact totals, exact item lists, item
  recall/precision, label similarity, merchant/date, latency p50/p95, tokens and cost. Photos are
  kept in the repo (owner's decision, 2026-10-31), with personal data cropped or blurred.

## Consequences

- Switching provider or model is a deployment setting; the harness tells which one to use.
- Strict schema support varies: some models may only work in `json_object` mode and need the
  repair retry more often. The report shows it.
- Every scan costs one or two model calls; the retry doubles the cost of bad answers only.
- The eval set grows by adding a photo and its JSON next to it; a test checks every ground truth
  parses and adds up.
- `cargo deny` now checks only the targets we build (server, apps, macOS CI), so wasm-only
  dependencies with other licences (bundled CA roots) don't count.
