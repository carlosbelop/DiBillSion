#!/bin/sh
# Runs in the `backup` container: one compressed dump a day, old ones pruned.
set -eu
while true; do
    file="/backups/dibill-$(date -u +%Y-%m-%dT%H%M).dump"
    if pg_dump --format=custom --file="$file.tmp"; then
        mv "$file.tmp" "$file"
        echo "backup written: $file"
    else
        rm -f "$file.tmp"
        echo "backup FAILED" >&2
    fi
    find /backups -name 'dibill-*.dump' -mtime "+$KEEP_DAYS" -delete
    sleep 86400
done
