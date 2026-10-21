#!/bin/sh
set -eu
umask 0077
mkdir -p /var/www/html/data /app/temp
chown www-data:www-data /var/www/html/data /app/temp
chmod 0700 /var/www/html/data
if [ -f /var/www/html/data/fakturacie.sqlite ]; then
    chown www-data:www-data /var/www/html/data/fakturacie.sqlite*
    chmod 0600 /var/www/html/data/fakturacie.sqlite*
fi
exec gosu www-data "$@"
