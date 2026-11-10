#!/bin/sh
# Forced command for the backup key (README, "Backups on another computer"): that key can only
# list the dumps or download one, nothing else. Installed in ~/dibillsion and named in
# ~/.ssh/authorized_keys as command="…/serve-backup.sh",restrict.
set -eu
dir="$(dirname "$0")/backups"
set -- ${SSH_ORIGINAL_COMMAND:-}
case "${1:-}" in
    list)
        cd "$dir" && ls -1 dibill-*.dump 2>/dev/null || true ;;
    get)
        name="${2:-}"
        case "$name" in
            dibill-[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9][0-9][0-9].dump) ;;
            *) echo "unknown dump" >&2; exit 1 ;;
        esac
        exec cat "$dir/$name" ;;
    *)
        echo "only 'list' and 'get <dump>' are allowed" >&2; exit 1 ;;
esac
