//! The evaluation set in `eval/receipts` is well formed: every image has a ground truth that
//! parses, follows the schema's conventions and adds up to the cent.

use std::path::{Path, PathBuf};

use dibill_domain::Currency;
use dibill_receipt::eval::score;
use dibill_receipt::{ModelReceipt, check, normalize, parse_date};

fn set() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../eval/receipts")
}

#[test]
fn every_image_has_a_valid_ground_truth() {
    let mut images = 0;
    for entry in std::fs::read_dir(set()).expect("eval/receipts exists") {
        let path = entry.expect("entry").path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        match ext {
            "jpg" | "jpeg" | "png" | "webp" => {
                images += 1;
                let truth_path = path.with_extension("json");
                let text = std::fs::read_to_string(&truth_path)
                    .unwrap_or_else(|_| panic!("{} has no ground truth", path.display()));
                let truth: ModelReceipt = serde_json::from_str(&text)
                    .unwrap_or_else(|e| panic!("{}: {e}", truth_path.display()));
                let currency = Currency::from_code(truth.currency.as_deref().unwrap_or("EUR"))
                    .expect("known currency");
                let name = truth_path.display();
                assert!(
                    check(&truth, currency).is_empty(),
                    "{name}: {:?}",
                    check(&truth, currency)
                );
                assert!(
                    normalize(&truth, currency).expect("normalizes").adds_up(),
                    "{name}"
                );
                if let Some(date) = &truth.date {
                    assert!(parse_date(date).is_some(), "{name}: bad date {date}");
                }
                for item in &truth.items {
                    assert!(
                        !item.total.contains(','),
                        "{name}: use a dot in {:?}",
                        item.total
                    );
                }
                assert!(score(&truth, Some(&truth)).items_exact, "{name}");
            }
            "json" => assert!(
                ["jpg", "jpeg", "png", "webp"]
                    .iter()
                    .any(|e| path.with_extension(e).exists()),
                "{} has no image",
                path.display()
            ),
            _ => {}
        }
    }
    assert!(images >= 5, "only {images} receipts");
}
