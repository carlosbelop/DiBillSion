//! Receipt scanning (ADR 0008): the photo goes to the server, which asks the vision model; the
//! answer is normalized here into minor units for the review screen. Saving is an ordinary
//! multi-item transaction once the user has assigned people to the items.

use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use dibill_proto::{ScanReceipt, ScannedReceipt};

use crate::sync::{json, parse, send_timed};
use crate::{Core, CoreError, GroupId, HttpTransport, uuid};

/// The server may ask the model twice (repair retry), each up to its own timeout.
const SCAN_TIMEOUT_SECS: u32 = 150;

/// One line of a scanned receipt, discounts already folded in. Never negative.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ScannedItemDto {
    pub label: String,
    pub quantity: Option<String>,
    pub total_minor: i64,
    /// Discounts folded into this item (zero or negative), to show next to it.
    pub discount_minor: i64,
}

/// A scanned receipt, in minor units of `currency`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ScannedReceiptDto {
    pub merchant: Option<String>,
    /// Days since 1970-01-01, when the receipt shows a date.
    pub date_days: Option<i32>,
    /// The receipt's currency, or the group's when it shows none.
    pub currency: String,
    pub items: Vec<ScannedItemDto>,
    /// The printed total, when readable.
    pub total_minor: Option<i64>,
    pub items_sum_minor: i64,
    pub scans_left_today: u32,
}

#[uniffi::export]
impl Core {
    /// Reads a receipt photo through the server. `image` is a JPEG, PNG or WebP (the apps send a
    /// ~1600 px JPEG); amounts are read in the group's currency unless the receipt shows another.
    /// The items may not add up to the total: compare `items_sum_minor` with `total_minor` and
    /// let the user fix it.
    ///
    /// # Errors
    /// [`CoreError::Offline`]; [`CoreError::Server`] with code `receipts_disabled`,
    /// `limit_reached`, `unreadable`, `provider_failed` or `too_large`.
    pub fn scan_receipt(
        &self,
        transport: Arc<dyn HttpTransport>,
        group_id: String,
        image: Vec<u8>,
        mime: String,
    ) -> Result<ScannedReceiptDto, CoreError> {
        let base = self.store().group(GroupId(uuid(&group_id)?))?.base_currency;
        let request = json(&ScanReceipt {
            image_base64: STANDARD.encode(&image),
            mime,
            currency: base.code().to_owned(),
        })?;
        let body = self.authed(&*transport, |token| {
            send_timed(
                &*transport,
                "POST",
                "/v1/receipts/scan".into(),
                Some(token),
                Some(request.clone()),
                SCAN_TIMEOUT_SECS,
            )
        })?;
        let scanned: ScannedReceipt = parse(&body)?;
        let receipt = dibill_receipt::normalize(&scanned.receipt, base).map_err(|issues| {
            CoreError::Server {
                code: "unreadable".into(),
                reason: issues
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
            }
        })?;
        Ok(ScannedReceiptDto {
            merchant: receipt.merchant,
            date_days: receipt.date.map(|(y, m, d)| days_from_civil(y, m, d)),
            currency: receipt.currency.code().to_owned(),
            items: receipt
                .items
                .into_iter()
                .map(|i| ScannedItemDto {
                    label: i.label,
                    quantity: i.quantity,
                    total_minor: i.total_minor,
                    discount_minor: i.discount_minor,
                })
                .collect(),
            total_minor: receipt.total_minor,
            items_sum_minor: receipt.items_sum_minor,
            scans_left_today: scanned.scans_left_today,
        })
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date (H. Hinnant's algorithm).
fn days_from_civil(year: i32, month: u8, day: u8) -> i32 {
    let (month, day) = (i32::from(month), i32::from(day));
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::{GroupKindDto, HttpRequest, HttpResponse, TransportError};

    /// Registers devices and answers scans with a fixed body.
    struct Fake {
        status: u16,
        answer: String,
        scans: Mutex<Vec<ScanReceipt>>,
    }

    impl HttpTransport for Fake {
        fn request(&self, r: HttpRequest) -> Result<HttpResponse, TransportError> {
            if r.path == "/v1/devices" {
                return Ok(HttpResponse {
                    status: 201,
                    body: r#"{"token":"t"}"#.into(),
                });
            }
            assert_eq!(
                (r.method.as_str(), r.path.as_str()),
                ("POST", "/v1/receipts/scan")
            );
            assert_eq!(r.token.as_deref(), Some("t"));
            assert_eq!(r.timeout_secs, SCAN_TIMEOUT_SECS);
            let req = serde_json::from_str(r.body.as_deref().unwrap_or_default()).unwrap();
            self.scans.lock().unwrap().push(req);
            Ok(HttpResponse {
                status: self.status,
                body: self.answer.clone(),
            })
        }
    }

    fn fake(status: u16, answer: &str) -> Arc<Fake> {
        Arc::new(Fake {
            status,
            answer: answer.into(),
            scans: Mutex::new(Vec::new()),
        })
    }

    fn group(core: &Core, currency: &str) -> String {
        core.create_group("Flat".into(), None, GroupKindDto::Home, currency.into())
            .unwrap()
    }

    #[test]
    fn normalizes_the_answer() {
        let core = Core::in_memory().unwrap();
        let g = group(&core, "EUR");
        let server = fake(
            200,
            r#"{"receipt":{"merchant":"AHORRAMAS","date":"2024-02-12","currency":null,"items":[
                {"description":"PAN","quantity":"1","unit_price":null,"total":"1.25"},
                {"description":"BARRA","quantity":"2","unit_price":"0.48","total":"0.96"},
                {"description":"PROMOCION","quantity":null,"unit_price":null,"total":"-0.08"}],
                "total":"2.13"},"scans_left_today":7}"#,
        );
        let scanned = core
            .scan_receipt(server.clone(), g, vec![1, 2, 3], "image/jpeg".into())
            .unwrap();
        assert_eq!(scanned.merchant.as_deref(), Some("AHORRAMAS"));
        assert_eq!(scanned.date_days, Some(19_765));
        assert_eq!(scanned.currency, "EUR");
        let totals: Vec<i64> = scanned.items.iter().map(|i| i.total_minor).collect();
        assert_eq!(totals, [125, 88]);
        assert_eq!(scanned.items[1].discount_minor, -8);
        assert_eq!(scanned.items[1].quantity.as_deref(), Some("2"));
        assert_eq!(
            (scanned.total_minor, scanned.items_sum_minor),
            (Some(213), 213)
        );
        assert_eq!(scanned.scans_left_today, 7);

        let sent = &server.scans.lock().unwrap()[0];
        assert_eq!(sent.image_base64, "AQID");
        assert_eq!(sent.currency, "EUR");
        assert_eq!(sent.mime, "image/jpeg");
    }

    #[test]
    fn errors_keep_the_server_code() {
        let core = Core::in_memory().unwrap();
        let g = group(&core, "EUR");
        let server = fake(
            429,
            r#"{"error":"limit_reached","message":"no scans left"}"#,
        );
        match core.scan_receipt(server, g.clone(), vec![1], "image/jpeg".into()) {
            Err(CoreError::Server { code, .. }) => assert_eq!(code, "limit_reached"),
            other => panic!("{other:?}"),
        }
        let server = fake(
            200,
            r#"{"receipt":{"merchant":null,"date":null,"currency":null,"items":[],"total":null},"scans_left_today":1}"#,
        );
        match core.scan_receipt(server, g, vec![1], "image/jpeg".into()) {
            Err(CoreError::Server { code, .. }) => assert_eq!(code, "unreadable"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn civil_days() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2024, 2, 12), 19_765);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(days_from_civil(1969, 12, 31), -1);
    }
}
