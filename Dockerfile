# syntax=docker/dockerfile:1
FROM node:24-bookworm-slim AS frontend
WORKDIR /build
COPY package.json package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY assets ./assets
COPY bin/build.mjs ./bin/build.mjs
RUN npm run build

FROM rust:1.99.1-bookworm AS build
RUN rustup component add clippy rustfmt
RUN apt-get update \
    && apt-get install -y --no-install-recommends libssl-dev liblzma-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY sql ./sql
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    cargo build --release --locked \
    && cp target/release/fakturacie /usr/local/bin/fakturacie

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates chromium fonts-dejavu-core libssl3 liblzma5 gosu \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /usr/local/bin/fakturacie /usr/local/bin/fakturacie
COPY --from=frontend /build/public/assets ./public/assets
COPY views ./views
COPY assets/invoice.css ./assets/invoice.css
COPY docker/entrypoint.sh /usr/local/bin/fakturacie-entrypoint
RUN mkdir -p /var/www/html/data /app/temp \
    && chown -R www-data:www-data /var/www/html/data /app/temp \
    && chmod 0755 /usr/local/bin/fakturacie-entrypoint
ENV FAKTURACIE_RUNTIME=docker \
    FAKTURACIE_ROOT=/app \
    FAKTURACIE_DB=/var/www/html/data/fakturacie.sqlite \
    CHROMIUM_BIN=/usr/bin/chromium
EXPOSE 8080
ENTRYPOINT ["fakturacie-entrypoint"]
CMD ["fakturacie"]

FROM build AS test
RUN apt-get update \
    && apt-get install -y --no-install-recommends chromium fonts-dejavu-core ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=frontend /build/public/assets ./public/assets
COPY views ./views
COPY assets/invoice.css ./assets/invoice.css
COPY tests ./tests
ENV FAKTURACIE_ROOT=/build CHROMIUM_BIN=/usr/bin/chromium FAKTURACIE_ARTIFACTS=/build/test-artifacts
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    cargo test --locked \
    && cargo clippy --locked --all-targets -- -D warnings \
    && cargo fmt --check

FROM runtime AS verify
USER root
RUN apt-get update \
    && apt-get install -y --no-install-recommends poppler-utils zbar-tools python3 \
    && rm -rf /var/lib/apt/lists/*
COPY tests ./tests
