//! Scores a model answer against the hand-written ground truth. Integers only: similarities are
//! per mille.

use dibill_domain::{Currency, Minor};

use crate::amount::parse_amount;
use crate::model::ModelReceipt;
use crate::normalize::check;

/// How one answer compares with the truth. Plain flags, one per metric in the report.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Score {
    /// The answer parsed as a receipt.
    pub parsed: bool,
    /// Its total equals the true total.
    pub total_ok: bool,
    /// Its own items add up to its own total (what the app can check without the truth).
    pub adds_up: bool,
    pub truth_items: usize,
    pub predicted_items: usize,
    /// True items found with the exact amount.
    pub matched_items: usize,
    /// Same items, same amounts, nothing extra.
    pub items_exact: bool,
    /// Sum over matched items of the description similarity, per mille each.
    pub label_similarity: u64,
    pub merchant_ok: bool,
    pub date_ok: bool,
}

/// Compares `predicted` with `truth`, amounts read in the truth's currency.
#[must_use]
pub fn score(truth: &ModelReceipt, predicted: Option<&ModelReceipt>) -> Score {
    let currency = truth
        .currency
        .as_deref()
        .and_then(|c| Currency::from_code(c).ok())
        .or_else(|| Currency::from_code("EUR").ok());
    let Some(currency) = currency else {
        return Score::default();
    };
    let exp = currency.exponent();
    let mut s = Score {
        truth_items: truth.items.len(),
        ..Score::default()
    };
    let Some(p) = predicted else { return s };
    s.parsed = true;
    s.predicted_items = p.items.len();

    let amount = |t: Option<&str>| t.and_then(|t| parse_amount(t, exp).ok());
    s.total_ok = amount(truth.total.as_deref()).is_some()
        && amount(truth.total.as_deref()) == amount(p.total.as_deref());
    s.adds_up = check(p, currency).is_empty();
    s.merchant_ok = match (&truth.merchant, &p.merchant) {
        (None, _) => true,
        (Some(t), Some(m)) => {
            similarity(t, m) >= 800 || normalize_label(m).contains(&normalize_label(t))
        }
        (Some(_), None) => false,
    };
    s.date_ok = truth.date.is_none()
        || truth.date.as_deref().map(str::trim) == p.date.as_deref().map(str::trim);

    // Greedy, in printed order: each true line takes the unused predicted line with the same
    // amount and the most similar description.
    let predicted: Vec<(Option<Minor>, &str)> = p
        .items
        .iter()
        .map(|i| (amount(Some(&i.total)), i.description.as_str()))
        .collect();
    let mut used = vec![false; predicted.len()];
    for item in &truth.items {
        let want = amount(Some(&item.total));
        let best = predicted
            .iter()
            .enumerate()
            .filter(|(i, (a, _))| !used[*i] && a.is_some() && *a == want)
            .map(|(i, (_, label))| (i, similarity(&item.description, label)))
            .max_by_key(|(i, sim)| (*sim, std::cmp::Reverse(*i)));
        if let Some((i, sim)) = best {
            used[i] = true;
            s.matched_items += 1;
            s.label_similarity += sim;
        }
    }
    s.items_exact = s.matched_items == s.truth_items && s.predicted_items == s.truth_items;
    s
}

/// Upper case, letters and digits only, single spaces.
fn normalize_label(text: &str) -> String {
    text.to_uppercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `1000 − 1000 · edit distance / longer length` on normalized labels.
#[must_use]
pub fn similarity(a: &str, b: &str) -> u64 {
    let a: Vec<char> = normalize_label(a).chars().collect();
    let b: Vec<char> = normalize_label(b).chars().collect();
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1000;
    }
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (diagonal + usize::from(ca != cb))
                .min(row[j] + 1)
                .min(row[j + 1] + 1);
            diagonal = row[j + 1];
            row[j + 1] = next;
        }
    }
    let distance = row[b.len()] as u64;
    1000 - 1000 * distance / longest as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModelItem;

    fn r(lines: &[(&str, &str)], total: &str) -> ModelReceipt {
        ModelReceipt {
            merchant: Some("AHORRAMAS".into()),
            date: Some("2024-02-12".into()),
            currency: Some("EUR".into()),
            items: lines
                .iter()
                .map(|(d, t)| ModelItem {
                    description: (*d).into(),
                    quantity: None,
                    unit_price: None,
                    total: (*t).into(),
                })
                .collect(),
            total: Some(total.into()),
        }
    }

    #[test]
    fn perfect_answer() {
        let truth = r(
            &[("PUERRO BANDEJA", "2.54"), ("ZANAHORIA 1KG", "1.15")],
            "3.69",
        );
        let s = score(&truth, Some(&truth));
        assert!(s.parsed && s.total_ok && s.adds_up && s.items_exact && s.merchant_ok && s.date_ok);
        assert_eq!(s.label_similarity, 2000);
    }

    #[test]
    fn partial_answer() {
        let truth = r(
            &[("PUERRO BANDEJA", "2.54"), ("ZANAHORIA 1KG", "1.15")],
            "3.69",
        );
        let mut answer = r(
            &[
                ("Puerro bandeja", "2,54"),
                ("ZANAHORA", "1.51"),
                ("BOLSA", "0.15"),
            ],
            "3.69",
        );
        answer.merchant = Some("Ahorramas S.A.".into());
        let s = score(&truth, Some(&answer));
        assert!(s.parsed && s.total_ok && !s.adds_up && !s.items_exact && s.merchant_ok);
        assert_eq!((s.matched_items, s.predicted_items), (1, 3));
        assert_eq!(s.label_similarity, 1000);
        assert_eq!(
            score(&truth, None),
            Score {
                truth_items: 2,
                ..Score::default()
            }
        );
    }

    #[test]
    fn similarities() {
        assert_eq!(
            similarity("CHOCO-LECHE ALMENDRA", "choco leche almendra"),
            1000
        );
        assert_eq!(similarity("", ""), 1000);
        assert_eq!(similarity("ABCD", "ABCE"), 750);
        assert_eq!(similarity("ABC", ""), 0);
    }
}
