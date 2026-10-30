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

## Operations

- Logs: `docker compose logs -f server`
- Backups: `~/dibillsion/backups/dibill-*.dump`, one a day, 14 days kept. Copy them off the host now
  and then (`scp`), a Pi's disk is a single point of failure.
- Restore: `docker compose stop server`, then
  `docker compose exec -T db pg_restore -U dibill -d dibill --clean --if-exists < backups/<file>.dump`,
  then `docker compose start server`.
- Update: run `deploy.sh` again; migrations run on start.
