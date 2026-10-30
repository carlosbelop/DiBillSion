#!/usr/bin/env bash
# Builds the server image here (cross-compiled for the target) and ships it to the
# beta host over SSH, then (re)starts the stack. The host needs Docker + compose and
# a deploy/.env with POSTGRES_PASSWORD.
#
#   DIBILL_HOST=pi@raspberrypi.local deploy/deploy.sh          # Raspberry Pi 4B (arm64)
#   DIBILL_HOST=root@1.2.3.4 DIBILL_PLATFORM=linux/amd64 deploy/deploy.sh
set -euo pipefail

host="${DIBILL_HOST:?set DIBILL_HOST, e.g. pi@raspberrypi.local}"
platform="${DIBILL_PLATFORM:-linux/arm64}"
remote_dir="${DIBILL_DIR:-dibillsion}"
tag="dibill-server:$(git rev-parse --short HEAD)"
root="$(git rev-parse --show-toplevel)"

echo "==> building $tag for $platform"
docker buildx build --platform "$platform" -f "$root/deploy/Dockerfile" -t "$tag" --load "$root"

echo "==> copying image and stack to $host:$remote_dir"
ssh "$host" "mkdir -p $remote_dir/backups"
docker save "$tag" | gzip | ssh "$host" "gunzip | docker load"
scp "$root/deploy/compose.yaml" "$root/deploy/backup.sh" "$host:$remote_dir/"
ssh "$host" "test -f $remote_dir/.env" || {
    echo "!! $host:$remote_dir/.env is missing: copy deploy/.env.example there and set POSTGRES_PASSWORD" >&2
    exit 1
}

echo "==> starting"
ssh "$host" "cd $remote_dir && DIBILL_IMAGE=$tag docker compose up -d --no-build && docker compose ps"
ssh "$host" "for i in \$(seq 1 30); do curl -fsS http://127.0.0.1:8080/health && exit 0; sleep 2; done; exit 1" \
    && echo && echo "==> healthy"
