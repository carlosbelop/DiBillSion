#!/usr/bin/env bash
# Copies the server's nightly dumps to this computer (a second place, in case the server's disk
# is lost) and keeps the newest $KEEP of them. Uses the restricted backup key, which can only
# download dumps (serve-backup.sh). Safe to run often: dumps already here are skipped.
#
#   DIBILL_HOST=carlosbl@37.27.32.157 DIBILL_BACKUP_DIR=/mnt/c/Users/me/DiBillSion-backups \
#     deploy/pull-backups.sh
set -euo pipefail

host="${DIBILL_HOST:?set DIBILL_HOST, e.g. carlosbl@37.27.32.157}"
dir="${DIBILL_BACKUP_DIR:?set DIBILL_BACKUP_DIR}"
key="${DIBILL_BACKUP_KEY:-$HOME/.ssh/dibillsion_backup}"
keep="${KEEP:-30}"
ssh_cmd=(ssh -i "$key" -o IdentitiesOnly=yes -o BatchMode=yes -o ConnectTimeout=20 "$host")

mkdir -p "$dir"
fetched=0
for name in $("${ssh_cmd[@]}" list); do
    [ -s "$dir/$name" ] && continue
    "${ssh_cmd[@]}" get "$name" > "$dir/$name.part"
    [ -s "$dir/$name.part" ] || { rm -f "$dir/$name.part"; echo "empty download: $name" >&2; exit 1; }
    mv "$dir/$name.part" "$dir/$name"
    fetched=$((fetched + 1))
done
# Newest names sort last (dibill-YYYY-MM-DDTHHMM.dump).
ls -1 "$dir"/dibill-*.dump 2>/dev/null | head -n "-$keep" | xargs -r rm -f
echo "$(date -Is) fetched $fetched, $(ls -1 "$dir"/dibill-*.dump 2>/dev/null | wc -l) kept in $dir"
