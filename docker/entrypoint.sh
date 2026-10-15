#!/bin/sh
set -eu
cd /var/www/html
mkdir -p temp/sessions temp/mpdf temp/latte-pdf log
php bin/setup.php
chown -R www-data:www-data data temp log
chmod 0700 data
chmod 0600 data/fakturacie.sqlite
exec docker-php-entrypoint "$@"
