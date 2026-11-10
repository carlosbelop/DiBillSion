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

A VPS is the same after installing Docker. The beta runs on a Hetzner CX23 (x86, Ubuntu 24.04),
prepared once as root: a sudo user with the SSH key (`carlosbl`), `PermitRootLogin no` and
`PasswordAuthentication no` in `/etc/ssh/sshd_config.d/`, `ufw` allowing only SSH, 80/tcp, 443/tcp and
443/udp, Docker from get.docker.com, unattended upgrades (Ubuntu's default). Deploy it with
`DIBILL_BUILD=zig DIBILL_PLATFORM=linux/amd64 DIBILL_HOST=carlosbl@<ip> deploy/deploy.sh`.

## 2. Deploy (from the dev machine)

```bash
DIBILL_HOST=pi@raspberrypi.local deploy/deploy.sh
```

It cross-compiles the image here, copies it over SSH, starts the stack and waits for `/health`.
Without Docker on the dev machine (WSL without Docker Desktop), add `DIBILL_BUILD=zig`: the
binary is cross-compiled with [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild)
(`cargo install cargo-zigbuild`, zig on `PATH`) and the host builds the image from it
(`Dockerfile.prebuilt`).
The server listens only on `127.0.0.1:8080` of the host; nothing is opened on the router.

## 3. HTTPS for the phones

**A VPS with its own domain (what the beta uses):** point a DNS `A` record (e.g. `api.example.com`,
"DNS only" if the DNS is on Cloudflare) at the server's IP, open ports 80 and 443, and add to the
host's `.env`:

```bash
COMPOSE_FILE=compose.yaml:compose.https.yaml
DIBILL_DOMAIN=api.example.com
```

`deploy.sh` then also starts Caddy ([`compose.https.yaml`](compose.https.yaml), [`Caddyfile`](Caddyfile)),
which gets and renews a Let's Encrypt certificate by itself and redirects HTTP to HTTPS.
`DIBILL_DOMAIN` may list several names, e.g. `api.example.com, example.com` (each needs its DNS record).

**Invite links.** With a public site, invites are `https://<site>/join/<code>`: chat apps show them as
links, and the apps open them directly when installed (Universal Links on iOS, App Links on Android);
otherwise the server's join page shows the code and how to get the app. In the host's `.env`:

```bash
DIBILL_SITE_URL=https://example.com                      # served by this server too (DIBILL_DOMAIN)
DIBILL_IOS_APP_IDS=ABCDE12345.com.dibillsion.app         # Apple Team ID + bundle id
DIBILL_ANDROID_APPS=com.dibillsion.app=AA:BB:…           # package = SHA-256 of the release signing cert
```

The server then serves `/join/<code>`, `/.well-known/apple-app-site-association` and
`/.well-known/assetlinks.json`. The iOS app declares `applinks:$(SITE_HOST)` and the Android app
an App Links filter for `dibill.siteHost` (both default to `dibillsion.com`).

**Without a public IP or domain (the Pi at home)**, pick one; both need no open ports and give a
valid certificate.

- **Tailscale Funnel** (free, no domain needed). On the host:
  `curl -fsSL https://tailscale.com/install.sh | sh`, `sudo tailscale up`, then
  `sudo tailscale funnel --bg 8080`. The URL is `https://<host>.<tailnet>.ts.net`.
- **Cloudflare Tunnel** (needs a domain whose DNS is on Cloudflare): `cloudflared tunnel login`,
  `cloudflared tunnel create dibill`, route `api.<domain>` to `http://localhost:8080`, and run it
  as a service.

That URL becomes the release build's `SYNC_URL`.

## 4. Receipt scanning (optional)

Scanning stays off until the server has a vision model provider ([ADR 0008](../docs/adr/0008-receipt-scanning.md)).
Add to `~/dibillsion/.env` on the host (only there: never in git), then run `deploy.sh` again.

Claude (Anthropic API key from the Claude Console, ideally a key just for this server, in a
workspace with a monthly spend limit):

```bash
RECEIPT_API=anthropic
RECEIPT_API_KEY=sk-ant-...                      # never leaves the server
RECEIPT_MODEL=claude-opus-5-5                   # or the model the eval picked
RECEIPT_DAILY_LIMIT=20                          # scans per phone in any 24 h
RECEIPT_MONTHLY_LIMIT=1000                      # scans per month for everyone: the budget cap
```

Any OpenAI-compatible provider instead (OpenAI, OpenRouter, Gemini's OpenAI endpoint...):

```bash
RECEIPT_API_BASE=https://openrouter.ai/api/v1
RECEIPT_API_KEY=...
RECEIPT_MODEL=google/gemini-2.5-flash
```

Optional: `RECEIPT_OUTPUT_MODE=json_object` for OpenAI-compatible models without strict JSON
Schema support, `RECEIPT_TIMEOUT_SECS` (default 60). The log says `receipt scanning on` with the model at start.
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

## Backups on another computer

The nightly dumps live on the server's own disk; a second copy elsewhere protects against losing it.
A dedicated key that can only download dumps (`serve-backup.sh` as its forced command) keeps that
copy current without a passphrase prompt:

1. On the computer: `ssh-keygen -t ed25519 -N "" -C "dibillsion backups (download only)" -f ~/.ssh/dibillsion_backup`.
2. On the server, add to `~/.ssh/authorized_keys`:
   `command="/home/<user>/dibillsion/serve-backup.sh",restrict ssh-ed25519 AAAA… dibillsion backups (download only)`.
   `restrict` turns off shells, port forwarding and the rest; the script only answers `list` and
   `get <dump>` (`deploy.sh` installs it).
3. Run [`pull-backups.sh`](pull-backups.sh) daily (`DIBILL_HOST`, `DIBILL_BACKUP_DIR`; keeps the newest 30).
   The beta's copy goes to `C:\Users\<you>\DiBillSion-backups` through a Windows scheduled task
   ("DiBillSion backups": daily at 21:00 and at log-on, catching up if the PC was off) that runs
   `wsl.exe -d Ubuntu --exec bash -lc "… ~/.local/bin/dibillsion-pull-backups >> …/pull.log"`.

The dumps hold every group's data (names, amounts): keep that folder private.
Restore one as in *Operations* above (`pg_restore` 17 or newer).
