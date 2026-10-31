//! Runs models over the receipt evaluation set and writes a markdown report.
//!
//! ```text
//! RECEIPT_API_KEY=... cargo run -p dibill-receipt --features eval --bin receipt-eval -- \
//!     --base-url https://openrouter.ai/api/v1 \
//!     --model google/gemini-2.5-flash --model openai/gpt-5-mini@0.25,2.00
//! ```
//!
//! `--model name@in,out` gives prices in USD per million input/output tokens, used when the
//! provider doesn't report the cost itself. Other options: `--mode json_schema|json_object|prompt`,
//! `--set <dir>` (default `eval/receipts`), `--only <name>` (repeatable), `--jobs <n>` (default 4),
//! `--out <dir>` (default `target/receipt-eval/<unix time>`), `--timeout <seconds>` (default 120).
//! Raw answers land in `<out>/<model>/<receipt>.txt`, the report in `<out>/report.md`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dibill_domain::Currency;
use dibill_receipt::eval::{Score, score};
use dibill_receipt::openai::OutputMode;
use dibill_receipt::{ModelReceipt, ProviderConfig, ScanOutcome, Scanner};
use rust_decimal::Decimal;
use tokio::sync::Semaphore;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone)]
struct ModelSpec {
    name: String,
    /// USD per million input and output tokens.
    prices: Option<(Decimal, Decimal)>,
}

#[derive(Debug)]
struct Args {
    base_url: String,
    models: Vec<ModelSpec>,
    mode: OutputMode,
    set: PathBuf,
    only: Vec<String>,
    jobs: usize,
    out: PathBuf,
    timeout: Duration,
}

#[derive(Debug)]
struct Case {
    name: String,
    image: PathBuf,
    mime: &'static str,
    truth: ModelReceipt,
}

#[derive(Debug)]
struct Run {
    case: String,
    score: Score,
    elapsed_ms: u128,
    input_tokens: u64,
    output_tokens: u64,
    cost: Option<Decimal>,
    attempts: usize,
    error: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = parse_args(std::env::args().skip(1).collect())?;
    let api_key = std::env::var("RECEIPT_API_KEY").map_err(|_| "RECEIPT_API_KEY is not set")?;
    let cases = load_cases(&args.set, &args.only)?;
    if cases.is_empty() {
        return Err(format!("no receipts in {}", args.set.display()).into());
    }
    std::fs::create_dir_all(&args.out)?;
    let cases = Arc::new(cases);

    let mut report = format!(
        "# Receipt eval\n\n{} receipts from `{}`, provider `{}`, mode {:?}.\n\n\
         | Model | Parsed | Total exact | Adds up | Items exact | Item recall | Item precision | Labels | Merchant | Date | Retries | p50 s | p95 s | Tokens in/out | Cost per receipt |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
        cases.len(),
        args.set.display(),
        args.base_url,
        args.mode
    );
    let mut details = String::new();
    for spec in &args.models {
        eprintln!("== {}", spec.name);
        let runs = run_model(&args, spec, &api_key, &cases).await?;
        report.push_str(&summary_row(spec, &runs));
        let _ = writeln!(
            details,
            "\n## {}\n\n| Receipt | Total | Adds up | Items | Labels | Time s | Note |\n|---|---|---|---|---|---|---|",
            spec.name
        );
        for r in &runs {
            let _ = writeln!(
                details,
                "| {} | {} | {} | {}/{} (+{}) | {} | {} | {} |",
                r.case,
                tick(r.score.total_ok),
                tick(r.score.adds_up),
                r.score.matched_items,
                r.score.truth_items,
                r.score
                    .predicted_items
                    .saturating_sub(r.score.matched_items),
                permille_avg(r.score.label_similarity, r.score.matched_items as u64),
                seconds(r.elapsed_ms),
                r.error
                    .as_deref()
                    .unwrap_or(if r.attempts > 1 { "retried" } else { "" }),
            );
        }
    }
    report.push_str(
        "\nTotal exact: the answer's total equals the truth. Adds up: its own items add up to its own total. \
         Items exact: same lines and amounts, nothing extra. Recall/precision: lines matched by exact amount. \
         Labels: description similarity of matched lines. Retries: answers that needed the repair round.\n",
    );
    report.push_str(&details);
    let path = args.out.join("report.md");
    std::fs::write(&path, &report)?;
    println!("{report}");
    eprintln!("report: {}", path.display());
    Ok(())
}

async fn run_model(
    args: &Args,
    spec: &ModelSpec,
    api_key: &str,
    cases: &Arc<Vec<Case>>,
) -> Result<Vec<Run>, Error> {
    let scanner = Scanner::new(ProviderConfig {
        base_url: args.base_url.clone(),
        api_key: api_key.to_owned(),
        model: spec.name.clone(),
        mode: args.mode,
        timeout: args.timeout,
    })?;
    let dir = args.out.join(spec.name.replace(['/', ':'], "_"));
    std::fs::create_dir_all(&dir)?;
    let permits = Arc::new(Semaphore::new(args.jobs));
    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..cases.len() {
        let (scanner, cases, permits, dir, spec) = (
            scanner.clone(),
            Arc::clone(cases),
            Arc::clone(&permits),
            dir.clone(),
            spec.clone(),
        );
        tasks.spawn(async move {
            let _permit = permits.acquire_owned().await;
            let case = &cases[index];
            let run = run_case(&scanner, &spec, case, &dir).await;
            eprintln!(
                "  {} {}",
                case.name,
                run.error.as_deref().unwrap_or(if run.score.items_exact {
                    "exact"
                } else {
                    "done"
                })
            );
            (index, run)
        });
    }
    let mut runs: Vec<(usize, Run)> = Vec::new();
    while let Some(joined) = tasks.join_next().await {
        runs.push(joined?);
    }
    runs.sort_by_key(|(i, _)| *i);
    Ok(runs.into_iter().map(|(_, r)| r).collect())
}

async fn run_case(scanner: &Scanner, spec: &ModelSpec, case: &Case, dir: &Path) -> Run {
    let fallback = case
        .truth
        .currency
        .as_deref()
        .and_then(|c| Currency::from_code(c).ok())
        .or_else(|| Currency::from_code("EUR").ok());
    let outcome = match (std::fs::read(&case.image), fallback) {
        (Ok(bytes), Some(fallback)) => scanner
            .scan(&bytes, case.mime, fallback)
            .await
            .map_err(|e| e.to_string()),
        (Err(e), _) => Err(e.to_string()),
        (_, None) => Err("no currency".to_owned()),
    };
    match outcome {
        Ok(ScanOutcome {
            receipt,
            raw,
            usage,
            elapsed,
        }) => {
            let _ = std::fs::write(
                dir.join(format!("{}.txt", case.name)),
                raw.join("\n\n--- repair ---\n\n"),
            );
            let reported: Option<Decimal> = if usage.cost_usd.is_empty() {
                None
            } else {
                usage
                    .cost_usd
                    .iter()
                    .map(|c| parse_decimal(c))
                    .sum::<Option<Decimal>>()
            };
            let cost = reported.or_else(|| {
                spec.prices.map(|(input, output)| {
                    (Decimal::from(usage.input_tokens) * input
                        + Decimal::from(usage.output_tokens) * output)
                        / Decimal::from(1_000_000)
                })
            });
            Run {
                case: case.name.clone(),
                score: score(&case.truth, receipt.as_ref()),
                elapsed_ms: elapsed.as_millis(),
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cost,
                attempts: raw.len(),
                error: receipt.is_none().then(|| "unparseable".to_owned()),
            }
        }
        Err(error) => Run {
            case: case.name.clone(),
            score: score(&case.truth, None),
            elapsed_ms: 0,
            input_tokens: 0,
            output_tokens: 0,
            cost: None,
            attempts: 0,
            error: Some(
                error
                    .chars()
                    .take(120)
                    .collect::<String>()
                    .replace('|', "/")
                    .replace('\n', " "),
            ),
        },
    }
}

fn summary_row(spec: &ModelSpec, runs: &[Run]) -> String {
    let n = runs.len();
    let count = |f: &dyn Fn(&Run) -> bool| runs.iter().filter(|r| f(r)).count();
    let truth: usize = runs.iter().map(|r| r.score.truth_items).sum();
    let predicted: usize = runs.iter().map(|r| r.score.predicted_items).sum();
    let matched: usize = runs.iter().map(|r| r.score.matched_items).sum();
    let similarity: u64 = runs.iter().map(|r| r.score.label_similarity).sum();
    let mut times: Vec<u128> = runs
        .iter()
        .filter(|r| r.error.is_none())
        .map(|r| r.elapsed_ms)
        .collect();
    times.sort_unstable();
    let percentile = |p: usize| {
        times
            .get((times.len() * p / 100).min(times.len().saturating_sub(1)))
            .copied()
    };
    let answered = runs.iter().filter(|r| r.attempts > 0).count().max(1) as u64;
    let costs: Vec<Decimal> = runs.iter().filter_map(|r| r.cost).collect();
    let cost = if costs.is_empty() {
        "–".to_owned()
    } else {
        format!(
            "${}",
            (costs.iter().sum::<Decimal>() / Decimal::from(costs.len())).round_dp(5)
        )
    };
    format!(
        "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {}/{} | {} |\n",
        spec.name,
        pct(count(&|r| r.score.parsed), n),
        pct(count(&|r| r.score.total_ok), n),
        pct(count(&|r| r.score.adds_up), n),
        pct(count(&|r| r.score.items_exact), n),
        pct(matched, truth),
        pct(matched, predicted),
        permille_avg(similarity, matched as u64),
        pct(count(&|r| r.score.parsed && r.score.merchant_ok), n),
        pct(count(&|r| r.score.parsed && r.score.date_ok), n),
        count(&|r| r.attempts > 1),
        percentile(50).map_or("–".to_owned(), seconds),
        percentile(95).map_or("–".to_owned(), seconds),
        runs.iter().map(|r| r.input_tokens).sum::<u64>() / answered,
        runs.iter().map(|r| r.output_tokens).sum::<u64>() / answered,
        cost,
    )
}

fn pct(part: usize, whole: usize) -> String {
    if whole == 0 {
        return "–".to_owned();
    }
    let tenths = part * 1000 / whole;
    format!("{}.{}%", tenths / 10, tenths % 10)
}

fn permille_avg(sum: u64, n: u64) -> String {
    if n == 0 {
        return "–".to_owned();
    }
    let avg = sum / n;
    format!("{}.{:03}", avg / 1000, avg % 1000)
}

fn seconds(ms: u128) -> String {
    format!("{}.{}", ms / 1000, (ms % 1000) / 100)
}

fn tick(ok: bool) -> &'static str {
    if ok { "✓" } else { "✗" }
}

fn parse_decimal(text: &str) -> Option<Decimal> {
    Decimal::from_str(text)
        .or_else(|_| Decimal::from_scientific(text))
        .ok()
}

fn load_cases(set: &Path, only: &[String]) -> Result<Vec<Case>, Error> {
    let mut cases = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(set)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    for image in entries {
        let mime = match image
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("jpg" | "jpeg") => "image/jpeg",
            Some("png") => "image/png",
            Some("webp") => "image/webp",
            _ => continue,
        };
        let name = image
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned();
        if !only.is_empty() && !only.contains(&name) {
            continue;
        }
        let truth_path = image.with_extension("json");
        let truth: ModelReceipt = serde_json::from_str(&std::fs::read_to_string(&truth_path)?)
            .map_err(|e| format!("{}: {e}", truth_path.display()))?;
        cases.push(Case {
            name,
            image,
            mime,
            truth,
        });
    }
    Ok(cases)
}

fn parse_args(raw: Vec<String>) -> Result<Args, Error> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut args = Args {
        base_url: String::new(),
        models: Vec::new(),
        mode: OutputMode::JsonSchema,
        set: PathBuf::from("eval/receipts"),
        only: Vec::new(),
        jobs: 4,
        out: PathBuf::from(format!("target/receipt-eval/{stamp}")),
        timeout: Duration::from_secs(120),
    };
    let mut it = raw.into_iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--base-url" => args.base_url = value()?,
            "--model" => {
                let v = value()?;
                let (name, prices) = match v.split_once('@') {
                    Some((name, prices)) => {
                        let (i, o) = prices
                            .split_once(',')
                            .ok_or("prices are --model name@in,out")?;
                        let parsed = parse_decimal(i)
                            .zip(parse_decimal(o))
                            .ok_or("prices must be decimals")?;
                        (name.to_owned(), Some(parsed))
                    }
                    None => (v, None),
                };
                args.models.push(ModelSpec { name, prices });
            }
            "--mode" => {
                args.mode = OutputMode::parse(&value()?)
                    .ok_or("--mode is json_schema, json_object or prompt")?;
            }
            "--set" => args.set = PathBuf::from(value()?),
            "--only" => args.only.push(value()?),
            "--jobs" => args.jobs = value()?.parse::<usize>()?.max(1),
            "--out" => args.out = PathBuf::from(value()?),
            "--timeout" => args.timeout = Duration::from_secs(value()?.parse()?),
            other => {
                return Err(format!(
                    "unknown option {other}; see the top of src/bin/receipt-eval.rs"
                )
                .into());
            }
        }
    }
    if args.base_url.is_empty() || args.models.is_empty() {
        return Err("--base-url and at least one --model are required".into());
    }
    Ok(args)
}
