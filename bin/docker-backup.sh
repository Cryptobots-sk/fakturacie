#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
umask 077
mkdir -p data/backups
temporary=$(mktemp data/backups/.docker-backup-XXXXXX)
trap 'rm -f "$temporary"' EXIT HUP INT TERM
docker compose exec -T --user www-data app php docker/database.php export > "$temporary"
backup="data/backups/docker-fakturacie-$(TZ=Europe/Bratislava date +%Y%m%d-%H%M%S)-$(basename "$temporary" | cut -c16-).sqlite"
mv "$temporary" "$backup"
printf 'Záloha: %s\n' "$backup"
