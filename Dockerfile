# syntax=docker/dockerfile:1
FROM php:8.4-apache-bookworm AS php-base
RUN apt-get update \
    && apt-get install -y --no-install-recommends libfreetype6-dev libjpeg62-turbo-dev libpng-dev libonig-dev libsqlite3-dev unzip \
    && docker-php-ext-configure gd --with-freetype --with-jpeg \
    && docker-php-ext-install -j"$(nproc)" bcmath gd mbstring pdo_sqlite opcache \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /var/www/html
COPY docker/php.ini /usr/local/etc/php/conf.d/fakturacie.ini
RUN cp "$PHP_INI_DIR/php.ini-production" "$PHP_INI_DIR/php.ini"

FROM php-base AS dependencies
COPY --from=composer:2 /usr/bin/composer /usr/local/bin/composer
COPY composer.json composer.lock ./
RUN --mount=type=cache,target=/root/.composer/cache composer install --no-dev --prefer-dist --no-interaction --no-progress --no-scripts --optimize-autoloader

FROM node:24-bookworm-slim AS frontend
WORKDIR /build
COPY package.json package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY assets ./assets
COPY bin/build.mjs ./bin/build.mjs
COPY --from=dependencies /var/www/html/vendor/mpdf/mpdf/ttfonts ./vendor/mpdf/mpdf/ttfonts
RUN npm run build

FROM php-base AS runtime
COPY --from=dependencies /var/www/html/vendor ./vendor
COPY --from=frontend /build/public/assets ./public/assets
COPY app ./app
COPY config ./config
COPY assets/invoice.css ./assets/invoice.css
COPY public/index.php public/router.php ./public/
COPY bin/backup.php bin/clear-cache.php bin/migrate.php bin/setup.php ./bin/
COPY docker/apache.conf /etc/apache2/sites-available/000-default.conf
COPY docker/entrypoint.sh /usr/local/bin/fakturacie-entrypoint
COPY docker/healthcheck.php docker/database.php ./docker/
RUN mkdir -p data temp log \
    && chown -R www-data:www-data data temp log \
    && chmod 0755 /usr/local/bin/fakturacie-entrypoint \
    && printf 'ServerName localhost\nServerTokens Prod\nServerSignature Off\n' > /etc/apache2/conf-available/fakturacie.conf \
    && a2enconf fakturacie
ENV FAKTURACIE_RUNTIME=docker
ENTRYPOINT ["fakturacie-entrypoint"]
CMD ["apache2-foreground"]

FROM runtime AS test
COPY --from=dependencies /usr/local/bin/composer /usr/local/bin/composer
COPY composer.json composer.lock phpstan.neon ./
RUN --mount=type=cache,target=/root/.composer/cache composer install --prefer-dist --no-interaction --no-progress --no-scripts --optimize-autoloader
COPY tests ./tests
COPY bin/setup.php ./bin/setup.php
