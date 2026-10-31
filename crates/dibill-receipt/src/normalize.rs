use dibill_domain::{Currency, Minor, allocate};

use crate::amount::{AmountError, parse_amount, parse_date};
use crate::model::ModelReceipt;

/// A scanned receipt in minor units, ready for the review screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub merchant: Option<String>,
    /// `(year, month, day)`.
    pub date: Option<(i32, u8, u8)>,
    /// The receipt's currency, or the fallback when it printed none or an unknown one.
    pub currency: Currency,
    /// Never negative: discount lines are folded into the items they apply to.
    pub items: Vec<ReceiptItem>,
    /// The printed total, when there is one.
    pub total_minor: Option<Minor>,
    pub items_sum_minor: Minor,
}

impl Receipt {
    /// The items add up to the printed total, to the cent.
    #[must_use]
    pub fn adds_up(&self) -> bool {
        self.total_minor == Some(self.items_sum_minor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptItem {
    pub label: String,
    pub quantity: Option<String>,
    /// After discounts.
    pub total_minor: Minor,
    /// Discounts folded into this item (zero or negative).
    pub discount_minor: Minor,
}

/// Something wrong with a model's answer. Drives the repair retry and the eval scores.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Issue {
    #[error("no items")]
    NoItems,
    #[error("line {line} ({description:?}) has an invalid total: {error}")]
    BadAmount {
        line: usize,
        description: String,
        error: AmountError,
    },
    #[error("the total is missing or invalid")]
    BadTotal,
    #[error("the item totals add up to {items_sum} but the total is {total}")]
    DoesNotAddUp { items_sum: String, total: String },
    #[error("the discounts are larger than the items they apply to")]
    DiscountTooLarge,
}

/// Every [`Issue`] with `receipt`, amounts read in `currency` unless the receipt names a known one.
#[must_use]
pub fn check(receipt: &ModelReceipt, fallback: Currency) -> Vec<Issue> {
    match normalize(receipt, fallback) {
        Ok(r) if r.total_minor.is_none() => vec![Issue::BadTotal],
        Ok(r) if !r.adds_up() => vec![Issue::DoesNotAddUp {
            items_sum: display(r.items_sum_minor, r.currency.exponent()),
            total: display(r.total_minor.unwrap_or(0), r.currency.exponent()),
        }],
        Ok(_) => vec![],
        Err(issues) => issues,
    }
}

/// Converts a model answer into minor units of its currency (or `fallback`), folding each
/// negative line into the nearest earlier item it fits in, and any that fit nowhere across all
/// items in proportion to their amounts (largest remainder, so nothing is lost or invented).
///
/// A missing or unreadable total is not an error here: [`Receipt::total_minor`] is `None` and
/// the review screen asks the user. A total that doesn't match the items isn't either:
/// [`Receipt::adds_up`] tells.
///
/// # Errors
/// The [`Issue`]s that make the items unusable.
pub fn normalize(receipt: &ModelReceipt, fallback: Currency) -> Result<Receipt, Vec<Issue>> {
    let currency = receipt
        .currency
        .as_deref()
        .and_then(|c| Currency::from_code(c.trim()).ok())
        .unwrap_or(fallback);
    let exponent = currency.exponent();

    let mut issues = Vec::new();
    let mut items: Vec<ReceiptItem> = Vec::new();
    let mut unplaced: Minor = 0;
    for (line, model) in receipt.items.iter().enumerate() {
        let amount = match parse_amount(&model.total, exponent) {
            Ok(a) => a,
            Err(error) => {
                issues.push(Issue::BadAmount {
                    line: line + 1,
                    description: model.description.clone(),
                    error,
                });
                continue;
            }
        };
        if amount >= 0 {
            items.push(ReceiptItem {
                label: model.description.trim().to_owned(),
                quantity: model.quantity.clone().filter(|q| !q.trim().is_empty()),
                total_minor: amount,
                discount_minor: 0,
            });
        } else if let Some(item) = items.iter_mut().rev().find(|i| i.total_minor >= -amount) {
            item.total_minor += amount;
            item.discount_minor += amount;
        } else {
            unplaced += amount;
        }
    }
    if items.is_empty() && issues.is_empty() {
        issues.push(Issue::NoItems);
    }
    if !issues.is_empty() {
        return Err(issues);
    }

    if unplaced < 0 {
        let amounts: Vec<Minor> = items.iter().map(|i| i.total_minor).collect();
        let sum: Minor = amounts.iter().sum();
        let spread =
            allocate(sum + unplaced, &amounts).map_err(|_| vec![Issue::DiscountTooLarge])?;
        for (item, new) in items.iter_mut().zip(spread) {
            item.discount_minor += new - item.total_minor;
            item.total_minor = new;
        }
    }

    let items_sum_minor = items.iter().map(|i| i.total_minor).sum();
    Ok(Receipt {
        merchant: receipt
            .merchant
            .as_deref()
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .map(str::to_owned),
        date: receipt.date.as_deref().and_then(parse_date),
        currency,
        items,
        total_minor: receipt
            .total
            .as_deref()
            .and_then(|t| parse_amount(t, exponent).ok()),
        items_sum_minor,
    })
}

/// `1234` with exponent 2 → `"12.34"`, for messages to the model.
fn display(minor: Minor, exponent: u8) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let abs = minor.unsigned_abs();
    if exponent == 0 {
        return format!("{sign}{abs}");
    }
    let scale = 10_u64.pow(u32::from(exponent));
    format!(
        "{sign}{}.{:0width$}",
        abs / scale,
        abs % scale,
        width = usize::from(exponent)
    )
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::model::ModelItem;

    fn eur() -> Currency {
        Currency::from_code("EUR").expect("EUR")
    }

    fn receipt(lines: &[&str], total: Option<&str>) -> ModelReceipt {
        ModelReceipt {
            merchant: Some(" Shop ".into()),
            date: Some("2024-02-12".into()),
            currency: Some("EUR".into()),
            items: lines
                .iter()
                .enumerate()
                .map(|(i, t)| ModelItem {
                    description: format!("item {i}"),
                    quantity: None,
                    unit_price: None,
                    total: (*t).to_owned(),
                })
                .collect(),
            total: total.map(str::to_owned),
        }
    }

    #[test]
    fn discount_folds_into_the_item_before_it() {
        let r = normalize(&receipt(&["1.29", "0.96", "-0.08"], Some("2.17")), eur()).expect("ok");
        let totals: Vec<Minor> = r.items.iter().map(|i| i.total_minor).collect();
        assert_eq!(totals, [129, 88]);
        assert_eq!(r.items[1].discount_minor, -8);
        assert!(r.adds_up());
        assert_eq!(r.merchant.as_deref(), Some("Shop"));
        assert_eq!(r.date, Some((2024, 2, 12)));
        assert_eq!(
            check(&receipt(&["1.29", "0.96", "-0.08"], Some("2.17")), eur()),
            []
        );
    }

    #[test]
    fn a_large_discount_skips_to_an_item_it_fits() {
        let r = normalize(&receipt(&["5.00", "0.50", "-1.00"], Some("4.50")), eur()).expect("ok");
        let totals: Vec<Minor> = r.items.iter().map(|i| i.total_minor).collect();
        assert_eq!(totals, [400, 50]);
    }

    #[test]
    fn a_leading_or_oversized_discount_is_spread() {
        let r = normalize(&receipt(&["-1.00", "3.00", "1.00"], Some("3.00")), eur()).expect("ok");
        let totals: Vec<Minor> = r.items.iter().map(|i| i.total_minor).collect();
        assert_eq!(totals, [225, 75]);
        assert_eq!(r.items_sum_minor, 300);
        assert_eq!(
            normalize(&receipt(&["1.00", "-2.00"], None), eur()),
            Err(vec![Issue::DiscountTooLarge])
        );
    }

    #[test]
    fn issues() {
        assert_eq!(
            check(&receipt(&["1.00", "2.00"], Some("3.50")), eur()),
            vec![Issue::DoesNotAddUp {
                items_sum: "3.00".into(),
                total: "3.50".into()
            }]
        );
        assert_eq!(
            check(&receipt(&["1.00"], None), eur()),
            vec![Issue::BadTotal]
        );
        assert_eq!(check(&receipt(&[], Some("0")), eur()), vec![Issue::NoItems]);
        assert!(matches!(
            check(&receipt(&["1.00", "Inv."], Some("1.00")), eur())[..],
            [Issue::BadAmount { line: 2, .. }]
        ));
    }

    #[test]
    fn unknown_currency_falls_back() {
        let mut r = receipt(&["100"], Some("100"));
        r.currency = Some("XYZ".into());
        let jpy = Currency::from_code("JPY").expect("JPY");
        let n = normalize(&r, jpy).expect("ok");
        assert_eq!(n.currency, jpy);
        assert_eq!(n.total_minor, Some(100));
    }

    #[test]
    fn display_amounts() {
        assert_eq!(display(-8, 2), "-0.08");
        assert_eq!(display(679_000, 2), "6790.00");
        assert_eq!(display(1234, 0), "1234");
        assert_eq!(display(1234, 3), "1.234");
    }

    proptest! {
        /// Folding moves money between items but never creates or loses any.
        #[test]
        fn folding_keeps_the_sum(lines in proptest::collection::vec(-500_i64..5000, 1..20)) {
            let texts: Vec<String> = lines.iter().map(|m| display(*m, 2)).collect();
            let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
            let sum: i64 = lines.iter().sum();
            match normalize(&receipt(&refs, None), eur()) {
                Ok(r) => {
                    prop_assert_eq!(r.items_sum_minor, sum);
                    prop_assert!(r.items.iter().all(|i| i.total_minor >= 0));
                }
                Err(e) => prop_assert!(
                    sum < 0 || lines.iter().all(|l| *l < 0) || e == vec![Issue::DiscountTooLarge],
                    "{e:?}"
                ),
            }
        }
    }
}
