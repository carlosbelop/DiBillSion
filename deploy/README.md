# Beta server

`dibill-server` + PostgreSQL 17 + nightly backups, for the friends beta (PLAN §4.1, B1).
The same stack runs on a Raspberry Pi 4B or a small VPS.

## 1. Prepare the host (once)

Raspberry Pi 4B:
- Raspberry Pi OS Lite **64-bit**. Prefer booting from a USB SSD: PostgreSQL writes wear out SD cards.
- Enable SSH with your key, then install Docker: `curl -fsSL https://get.docker.com | sh` and
  `sudo usermod -aG docker $USER` (log out and in).
- `mkdir -p ~/dibillsion` and create `~/dibillsion/.env` from [`.env.example`](.env.example):
  `POSTGRES_PASSWORD=$(openssl rand -hex 24)`.

A VPS (e.g. Hetzner CAX11, also ARM64) is the same after installing Docker.

## 2. Deploy (from the dev machine)

```bash
DIBILL_HOST=pi@raspberrypi.local deploy/deploy.sh
```

It cross-compiles the image here, copies it over SSH, starts the stack and waits for `/health`.
The server listens only on `127.0.0.1:8080` of the host; nothing is opened on the router.

## 3. HTTPS for the phones

Pick one; both need no open ports and give a valid certificate.

- **Tailscale Funnel** (free, no domain needed). On the host:
  `curl -fsSL https://tailscale.com/install.sh | sh`, `sudo tailscale up`, then
  `sudo tailscale funnel --bg 8080`. The URL is `https://<host>.<tailnet>.ts.net`.
- **Cloudflare Tunnel** (needs a domain whose DNS is on Cloudflare): `cloudflared tunnel login`,
  `cloudflared tunnel create dibill`, route `api.<domain>` to `http://localhost:8080`, and run it
  as a service.

That URL becomes the release build's `SYNC_URL`.

## 4. Receipt scanning (optional)

Scanning stays off until the server has a vision model provider ([ADR 0008](../docs/adr/0008-receipt-scanning.md)).
Any OpenAI-compatible endpoint works. Add to `~/dibillsion/.env` on the host, then run `deploy.sh` again:

```bash
RECEIPT_API_BASE=https://openrouter.ai/api/v1   # or https://api.openai.com/v1, Gemini's OpenAI endpoint...
RECEIPT_API_KEY=...                             # the provider key; it never leaves the server
RECEIPT_MODEL=google/gemini-2.5-flash           # the model the eval picked
RECEIPT_DAILY_LIMIT=20                          # scans per phone in any 24 h
RECEIPT_MONTHLY_LIMIT=1000                      # scans per month for everyone: the budget cap
```

Optional: `RECEIPT_OUTPUT_MODE=json_object` for models without strict JSON Schema support,
`RECEIPT_TIMEOUT_SECS` (default 60). The log says `receipt scanning on` with the model at start.
Each scan is a row in `receipt_scans` (time, model, tokens, no image):
`docker compose exec db psql -U dibill -c "SELECT date_trunc('day', created_at) d, count(*), sum(input_tokens), sum(output_tokens) FROM receipt_scans GROUP BY 1 ORDER BY 1"`.

## Operations

- Logs: `docker compose logs -f server`
- Backups: `~/dibillsion/backups/dibill-*.dump`, one a day, 14 days kept. Copy them off the host now
  and then (`scp`), a Pi's disk is a single point of failure.
- Restore: `docker compose stop server`, then
  `docker compose exec -T db pg_restore -U dibill -d dibill --clean --if-exists < backups/<file>.dump`,
  then `docker compose start server`.
- Update: run `deploy.sh` again; migrations run on start.
