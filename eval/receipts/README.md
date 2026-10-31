# Receipt evaluation set

Photos of real receipts, each next to a hand-written ground truth (`<name>.json`) in exactly the
shape the model must return (see `crates/dibill-receipt`, [ADR 0008](../../docs/adr/0008-receipt-scanning.md)).
The harness runs every model on every image here and scores it against these files:

```bash
RECEIPT_API_KEY=... cargo run -p dibill-receipt --features eval --bin receipt-eval -- \
  --base-url https://openrouter.ai/api/v1 --model google/gemini-2.5-flash --model openai/gpt-5-mini
```

## Adding a receipt

1. Drop the photo here as `<country>-<shop>-<year>.<jpg|png|webp>` (lower-case, dashes).
   Crop or blur anything personal first: card numbers, names, loyalty ids, addresses.
2. Write `<same name>.json` by reading the paper (not the model's output):
   - `description` as printed (keep upper case, abbreviations and accents).
   - Amounts as decimal strings with a dot (`"2.50"`, `"-0.08"`), exactly as printed.
   - `quantity` and `unit_price` when the receipt shows them; `"1"` and `null` otherwise.
   - One entry per printed line, in order. Discounts are their own negative lines; free items
     (`Inv.`, `0,00`) are `"0.00"`. Tax already included in the prices is not a line; tax, tip or
     service added on top of the items is.
   - `total` is the amount paid. The item totals must add up to it exactly.
   - `merchant`, `date` (`YYYY-MM-DD`) and `currency` (ISO 4217) as printed, `null` if not visible.
3. `cargo nextest run -p dibill-receipt` checks that every file parses and adds up.

## Notes per receipt

| File | Notes |
|---|---|
| `es-supermarket-valencia-2022` | Shop name cut off at the top (`merchant: null`). Creased paper. |
| `es-ahorramas-2024` | Multi-quantity lines on two rows (`2 Un x 1,25`); a `PROMOCION -0,08` discount line. |
| `es-carrefour-campanar` | Bottom cut off: only `SUBTOTAL 75,35` is visible, used as the total. No date. One line crossed out by hand (still part of the receipt). |
| `fr-relais-odeon-2017` | French restaurant, TVA included. Low resolution, glare. |
| `es-hermanos-alba-2025` | Restaurant with free lines (`Inv.`), multi-quantity lines without unit price, large amounts. Small photo. |
