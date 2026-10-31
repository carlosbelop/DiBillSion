use dibill_domain::Minor;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AmountError {
    #[error("not an amount: {0:?}")]
    Malformed(String),
    #[error("more decimals than the currency has: {0:?}")]
    TooPrecise(String),
    #[error("amount too large: {0:?}")]
    Overflow(String),
}

/// Parses a printed amount into minor units of a currency with `exponent` decimals, in integers.
///
/// Accepts what models and receipts produce: `"2.50"`, `"2,50"`, `"-0.08"`, `"0,08-"`,
/// `"1.234,56"`, `"1,234.56"`, `"€ 6790,00"`. With both separators the last one is the decimal
/// point; a lone separator is a decimal point unless it repeats (`"1.234.567"`). Extra decimals
/// are accepted only when they are zeros.
///
/// # Errors
/// [`AmountError`] when the text isn't a single amount, needs more decimals than the currency
/// has, or doesn't fit in [`Minor`].
pub fn parse_amount(text: &str, exponent: u8) -> Result<Minor, AmountError> {
    let malformed = || AmountError::Malformed(text.to_owned());
    let mut s: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && !"€$£¥".contains(*c))
        .collect();
    let mut negative = false;
    if let Some(rest) = s.strip_prefix('-') {
        negative = true;
        s = rest.to_owned();
    } else if let Some(rest) = s.strip_suffix('-') {
        negative = true;
        s = rest.to_owned();
    }
    if s.is_empty()
        || !s
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == ',')
    {
        return Err(malformed());
    }

    let decimal_at = match (s.rfind('.'), s.rfind(',')) {
        (Some(d), Some(c)) => Some(d.max(c)),
        (Some(i), None) | (None, Some(i)) => {
            let sep = s.as_bytes()[i];
            (s.bytes().filter(|b| *b == sep).count() == 1).then_some(i)
        }
        (None, None) => None,
    };
    let (whole, fraction) = match decimal_at {
        Some(i) => (&s[..i], &s[i + 1..]),
        None => (s.as_str(), ""),
    };
    // Only the other separator may appear before the decimal point, as thousands grouping.
    let decimal_sep = decimal_at.map(|i| s.as_bytes()[i] as char);
    if fraction.contains(['.', ',']) || decimal_sep.is_some_and(|d| whole.contains(d)) {
        return Err(malformed());
    }
    let whole: String = whole.chars().filter(char::is_ascii_digit).collect();
    if whole.is_empty() && fraction.is_empty() {
        return Err(malformed());
    }

    let exp = usize::from(exponent);
    let significant = fraction.trim_end_matches('0').len();
    if significant > exp {
        return Err(AmountError::TooPrecise(text.to_owned()));
    }
    let mut digits = whole;
    digits.push_str(&fraction[..fraction.len().min(exp)]);
    digits.extend(std::iter::repeat_n('0', exp.saturating_sub(fraction.len())));
    let value: Minor = digits.trim_start_matches('0').parse().or_else(|_| {
        if digits.trim_start_matches('0').is_empty() {
            Ok(0)
        } else {
            Err(AmountError::Overflow(text.to_owned()))
        }
    })?;
    Ok(if negative { -value } else { value })
}

/// Validates a `YYYY-MM-DD` date and returns `(year, month, day)`.
#[must_use]
pub fn parse_date(text: &str) -> Option<(i32, u8, u8)> {
    let mut parts = text.trim().splitn(3, '-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let (y, m, d): (i32, u8, u8) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    (1..=days).contains(&d).then_some((y, m, d))
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn printed_formats() {
        let cases = [
            ("2.50", 250),
            ("2,50", 250),
            ("2.5", 250),
            ("-0.08", -8),
            ("0,08-", -8),
            ("€ 6790,00", 679_000),
            ("1.234,56", 123_456),
            ("1,234.56", 123_456),
            ("1.234.567", 123_456_700),
            ("12", 1200),
            (".5", 50),
            ("2.500", 250),
            ("0", 0),
            ("0.00", 0),
        ];
        for (text, minor) in cases {
            assert_eq!(parse_amount(text, 2), Ok(minor), "{text}");
        }
        assert_eq!(parse_amount("1234", 0), Ok(1234));
        assert_eq!(parse_amount("1.234", 3), Ok(1234));
    }

    #[test]
    fn rejects() {
        for text in ["", "-", "abc", "1.2.3,4,5", "Inv.", "1e3", "--1"] {
            assert!(parse_amount(text, 2).is_err(), "{text}");
        }
        assert_eq!(
            parse_amount("0.625", 2),
            Err(AmountError::TooPrecise("0.625".into()))
        );
        assert!(matches!(
            parse_amount("99999999999999999999", 2),
            Err(AmountError::Overflow(_))
        ));
    }

    #[test]
    fn dates() {
        assert_eq!(parse_date("2024-02-12"), Some((2024, 2, 12)));
        assert_eq!(parse_date("2024-02-29"), Some((2024, 2, 29)));
        assert_eq!(parse_date("2023-02-29"), None);
        assert_eq!(parse_date("12/02/2024"), None);
        assert_eq!(parse_date("2024-13-01"), None);
    }

    proptest! {
        #[test]
        fn round_trips_cents(minor in -1_000_000_000_i64..1_000_000_000) {
            let text = format!("{}{}.{:02}", if minor < 0 { "-" } else { "" }, minor.abs() / 100, minor.abs() % 100);
            prop_assert_eq!(parse_amount(&text, 2), Ok(minor));
            prop_assert_eq!(parse_amount(&text.replace('.', ","), 2), Ok(minor));
        }
    }
}
