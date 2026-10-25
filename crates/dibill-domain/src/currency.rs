use std::fmt;
use std::str::FromStr;

use rust_decimal::{Decimal, RoundingStrategy};

use crate::Minor;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CurrencyError {
    #[error("unknown ISO 4217 currency code: {0:?}")]
    UnknownCode(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FxError {
    #[error("exchange rate must be greater than zero")]
    NonPositiveRate,
    #[error("exchange rate is not a valid decimal number")]
    InvalidRate,
    #[error("converted amount does not fit in minor units")]
    Overflow,
}

/// An ISO 4217 currency: three-letter code plus the number of decimal digits
/// of its minor unit (EUR 2, JPY 0, KWD 3, CLF 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Currency {
    code: [u8; 3],
    exponent: u8,
}

impl Currency {
    /// Looks up an active ISO 4217 currency by its upper-case code.
    ///
    /// # Errors
    /// [`CurrencyError::UnknownCode`] if the code is not in the table. Codes
    /// without a minor unit (precious metals, XDR, testing codes) are excluded.
    pub fn from_code(code: &str) -> Result<Self, CurrencyError> {
        ISO_4217
            .binary_search_by(|(c, _)| (*c).cmp(code))
            .map(|i| {
                let (c, exponent) = ISO_4217[i];
                let b = c.as_bytes();
                Self {
                    code: [b[0], b[1], b[2]],
                    exponent,
                }
            })
            .map_err(|_| CurrencyError::UnknownCode(code.to_owned()))
    }

    #[must_use]
    pub fn code(&self) -> &str {
        // Codes only come from the ASCII table above.
        std::str::from_utf8(&self.code).unwrap_or("???")
    }

    /// Decimal digits of the minor unit: 1 major unit = 10^exponent minor units.
    #[must_use]
    pub fn exponent(&self) -> u8 {
        self.exponent
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl FromStr for Currency {
    type Err = CurrencyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_code(s)
    }
}

/// Exchange rate between two currencies, in major units:
/// 1 unit of the source currency = `rate` units of the target currency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FxRate(Decimal);

impl FxRate {
    /// # Errors
    /// [`FxError::NonPositiveRate`] if `rate <= 0`.
    pub fn new(rate: Decimal) -> Result<Self, FxError> {
        if rate > Decimal::ZERO {
            Ok(Self(rate.normalize()))
        } else {
            Err(FxError::NonPositiveRate)
        }
    }

    #[must_use]
    pub fn value(&self) -> Decimal {
        self.0
    }
}

impl FromStr for FxRate {
    type Err = FxError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(Decimal::from_str_exact(s).map_err(|_| FxError::InvalidRate)?)
    }
}

/// Converts `amount` minor units of `from` into minor units of `to`.
///
/// The exact product is computed in decimal and rounded **once**, to the
/// nearest minor unit of `to`, with halves rounded away from zero. That makes
/// the conversion symmetric for refunds: `convert(-a) == -convert(a)`.
///
/// # Errors
/// [`FxError::Overflow`] if the result does not fit in [`Minor`].
pub fn convert(
    amount: Minor,
    from: Currency,
    to: Currency,
    rate: FxRate,
) -> Result<Minor, FxError> {
    let shift = i32::from(to.exponent) - i32::from(from.exponent);
    let scale = Decimal::from(10_i64.pow(shift.unsigned_abs()));
    let product = Decimal::from(amount)
        .checked_mul(rate.0)
        .ok_or(FxError::Overflow)?;
    let shifted = if shift >= 0 {
        product.checked_mul(scale)
    } else {
        product.checked_div(scale)
    }
    .ok_or(FxError::Overflow)?;
    let rounded = shifted.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
    Minor::try_from(rounded).map_err(|_| FxError::Overflow)
}

/// Active ISO 4217 codes with a minor unit, sorted by code.
#[rustfmt::skip]
const ISO_4217: &[(&str, u8)] = &[
    ("AED", 2), ("AFN", 2), ("ALL", 2), ("AMD", 2), ("AOA", 2), ("ARS", 2), ("AUD", 2),
    ("AWG", 2), ("AZN", 2), ("BAM", 2), ("BBD", 2), ("BDT", 2), ("BGN", 2), ("BHD", 3),
    ("BIF", 0), ("BMD", 2), ("BND", 2), ("BOB", 2), ("BOV", 2), ("BRL", 2), ("BSD", 2),
    ("BTN", 2), ("BWP", 2), ("BYN", 2), ("BZD", 2), ("CAD", 2), ("CDF", 2), ("CHE", 2),
    ("CHF", 2), ("CHW", 2), ("CLF", 4), ("CLP", 0), ("CNY", 2), ("COP", 2), ("COU", 2),
    ("CRC", 2), ("CUP", 2), ("CVE", 2), ("CZK", 2), ("DJF", 0), ("DKK", 2), ("DOP", 2),
    ("DZD", 2), ("EGP", 2), ("ERN", 2), ("ETB", 2), ("EUR", 2), ("FJD", 2), ("FKP", 2),
    ("GBP", 2), ("GEL", 2), ("GHS", 2), ("GIP", 2), ("GMD", 2), ("GNF", 0), ("GTQ", 2),
    ("GYD", 2), ("HKD", 2), ("HNL", 2), ("HTG", 2), ("HUF", 2), ("IDR", 2), ("ILS", 2),
    ("INR", 2), ("IQD", 3), ("IRR", 2), ("ISK", 0), ("JMD", 2), ("JOD", 3), ("JPY", 0),
    ("KES", 2), ("KGS", 2), ("KHR", 2), ("KMF", 0), ("KPW", 2), ("KRW", 0), ("KWD", 3),
    ("KYD", 2), ("KZT", 2), ("LAK", 2), ("LBP", 2), ("LKR", 2), ("LRD", 2), ("LSL", 2),
    ("LYD", 3), ("MAD", 2), ("MDL", 2), ("MGA", 2), ("MKD", 2), ("MMK", 2), ("MNT", 2),
    ("MOP", 2), ("MRU", 2), ("MUR", 2), ("MVR", 2), ("MWK", 2), ("MXN", 2), ("MXV", 2),
    ("MYR", 2), ("MZN", 2), ("NAD", 2), ("NGN", 2), ("NIO", 2), ("NOK", 2), ("NPR", 2),
    ("NZD", 2), ("OMR", 3), ("PAB", 2), ("PEN", 2), ("PGK", 2), ("PHP", 2), ("PKR", 2),
    ("PLN", 2), ("PYG", 0), ("QAR", 2), ("RON", 2), ("RSD", 2), ("RUB", 2), ("RWF", 0),
    ("SAR", 2), ("SBD", 2), ("SCR", 2), ("SDG", 2), ("SEK", 2), ("SGD", 2), ("SHP", 2),
    ("SLE", 2), ("SOS", 2), ("SRD", 2), ("SSP", 2), ("STN", 2), ("SVC", 2), ("SYP", 2),
    ("SZL", 2), ("THB", 2), ("TJS", 2), ("TMT", 2), ("TND", 3), ("TOP", 2), ("TRY", 2),
    ("TTD", 2), ("TWD", 2), ("TZS", 2), ("UAH", 2), ("UGX", 0), ("USD", 2), ("USN", 2),
    ("UYI", 0), ("UYU", 2), ("UYW", 4), ("UZS", 2), ("VED", 2), ("VES", 2), ("VND", 0),
    ("VUV", 0), ("WST", 2), ("XAF", 0), ("XCD", 2), ("XCG", 2), ("XOF", 0), ("XPF", 0),
    ("YER", 2), ("ZAR", 2), ("ZMW", 2), ("ZWG", 2),
];

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn cur(code: &str) -> Currency {
        Currency::from_code(code).unwrap()
    }

    fn rate(s: &str) -> FxRate {
        s.parse().unwrap()
    }

    #[test]
    fn table_is_sorted_unique_and_well_formed() {
        assert!(ISO_4217.windows(2).all(|w| w[0].0 < w[1].0));
        for (code, exp) in ISO_4217 {
            assert!(code.len() == 3 && code.bytes().all(|b| b.is_ascii_uppercase()));
            assert!(*exp <= 4);
        }
    }

    #[test]
    fn lookup() {
        assert_eq!(cur("EUR").exponent(), 2);
        assert_eq!(cur("JPY").exponent(), 0);
        assert_eq!(cur("KWD").exponent(), 3);
        assert_eq!(cur("CLF").exponent(), 4);
        assert_eq!(cur("USD").to_string(), "USD");
        assert_eq!("GBP".parse::<Currency>().unwrap().code(), "GBP");
        assert_eq!(
            Currency::from_code("eur"),
            Err(CurrencyError::UnknownCode("eur".into()))
        );
        assert!(Currency::from_code("XAU").is_err());
        assert!(Currency::from_code("").is_err());
    }

    #[test]
    fn rates_must_be_positive_decimals() {
        assert_eq!("0".parse::<FxRate>(), Err(FxError::NonPositiveRate));
        assert_eq!("-1.2".parse::<FxRate>(), Err(FxError::NonPositiveRate));
        assert_eq!("abc".parse::<FxRate>(), Err(FxError::InvalidRate));
        assert_eq!(rate("1.0850").value(), Decimal::new(1085, 3));
    }

    #[test]
    fn worked_examples() {
        // 10.00 USD at 0.9215 EUR/USD = 9.215 EUR -> 9.22 (half away from zero).
        assert_eq!(
            convert(1000, cur("USD"), cur("EUR"), rate("0.9215")),
            Ok(922)
        );
        assert_eq!(
            convert(-1000, cur("USD"), cur("EUR"), rate("0.9215")),
            Ok(-922)
        );
        // 1,234 JPY at 0.0061 EUR/JPY = 7.5274 EUR -> 7.53.
        assert_eq!(
            convert(1234, cur("JPY"), cur("EUR"), rate("0.0061")),
            Ok(753)
        );
        // 12.34 EUR at 162.5 JPY/EUR = 2005.25 JPY -> 2005.
        assert_eq!(
            convert(1234, cur("EUR"), cur("JPY"), rate("162.5")),
            Ok(2005)
        );
        // 1.000 KWD at 3.0 EUR/KWD = 3.00 EUR (fewer minor digits).
        assert_eq!(convert(1000, cur("KWD"), cur("EUR"), rate("3")), Ok(300));
        // 0.005 KWD at 1 -> 0.005 EUR rounds to 0.01 EUR.
        assert_eq!(convert(5, cur("KWD"), cur("EUR"), rate("1")), Ok(1));
        assert_eq!(convert(0, cur("EUR"), cur("USD"), rate("1.1")), Ok(0));
    }

    #[test]
    fn overflow_is_an_error() {
        assert_eq!(
            convert(Minor::MAX, cur("EUR"), cur("EUR"), rate("2")),
            Err(FxError::Overflow)
        );
        assert_eq!(
            convert(Minor::MAX / 10, cur("JPY"), cur("CLF"), rate("1")),
            Err(FxError::Overflow)
        );
    }

    fn any_currency() -> impl Strategy<Value = Currency> {
        (0..ISO_4217.len()).prop_map(|i| cur(ISO_4217[i].0))
    }

    fn any_rate() -> impl Strategy<Value = FxRate> {
        // 0.000001 ..= 100000 with up to 6 decimal places.
        (1i64..100_000_000_000).prop_map(|m| FxRate::new(Decimal::new(m, 6)).unwrap())
    }

    // Amounts up to 10^9 minor units with rates up to 10^5 and a 4-digit
    // exponent shift stay below i64::MAX, so no case overflows.
    proptest! {
        #[test]
        fn same_currency_rate_one_is_identity(amount in -1_000_000_000i64..1_000_000_000, c in any_currency()) {
            prop_assert_eq!(convert(amount, c, c, FxRate::new(Decimal::ONE).unwrap()), Ok(amount));
        }

        #[test]
        fn symmetric_for_refunds(amount in 0i64..1_000_000_000, from in any_currency(), to in any_currency(), r in any_rate()) {
            let pos = convert(amount, from, to, r).unwrap();
            prop_assert_eq!(convert(-amount, from, to, r).unwrap(), -pos);
        }

        #[test]
        fn within_half_a_minor_unit(amount in -1_000_000_000i64..1_000_000_000, from in any_currency(), to in any_currency(), r in any_rate()) {
            let got = convert(amount, from, to, r).unwrap();
            let shift = i32::from(to.exponent()) - i32::from(from.exponent());
            let mut exact = Decimal::from(amount) * r.value();
            exact = if shift >= 0 {
                exact * Decimal::from(10_i64.pow(shift.unsigned_abs()))
            } else {
                exact / Decimal::from(10_i64.pow(shift.unsigned_abs()))
            };
            prop_assert!((Decimal::from(got) - exact).abs() <= Decimal::new(5, 1));
        }

        #[test]
        fn monotonic(a in 0i64..1_000_000_000, b in 0i64..1_000_000_000, from in any_currency(), to in any_currency(), r in any_rate()) {
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            prop_assert!(convert(lo, from, to, r).unwrap() <= convert(hi, from, to, r).unwrap());
        }
    }
}
