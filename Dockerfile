# Build-time downloads only; runtime assets and browser are local.
FROM node:22-bookworm-slim AS frontend
WORKDIR /src
COPY package.json package-lock.json ./
RUN npm ci --ignore-scripts
COPY scripts/vendor_editor.mjs scripts/vendor_editor.mjs
COPY assets assets
COPY public public
RUN npm run vendor
ENV PLAYWRIGHT_BROWSERS_PATH=/opt/browsers
RUN npx playwright install --with-deps chromium

FROM rust:1.91.1-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends clang pkg-config && rm -rf /var/lib/apt/lists/*
RUN rustup target add wasm32-unknown-unknown && cargo install cargo-leptos --version 0.3.6 --locked
WORKDIR /src
COPY . .
COPY --from=frontend /src/public public
COPY --from=frontend /src/assets/vendor/codemirror assets/vendor/codemirror
RUN cargo leptos build --release && cargo build --release --locked -p mdpdf-cli

FROM node:22-bookworm-slim AS runtime
ENV PLAYWRIGHT_BROWSERS_PATH=/opt/browsers
# Install only browser OS dependencies; do not retain npm or source dependencies.
COPY --from=frontend /src/node_modules/playwright-core /tmp/playwright-core
RUN node /tmp/playwright-core/cli.js install-deps chromium && apt-get update && apt-get install -y --no-install-recommends fonts-noto-color-emoji ca-certificates tini && rm -rf /var/lib/apt/lists/* /tmp/playwright-core
COPY --from=frontend /opt/browsers/chromium-1248/chrome-linux64 /opt/chromium
RUN groupadd --gid 10001 mdpdf && useradd --uid 10001 --gid mdpdf --create-home mdpdf
WORKDIR /app
COPY --from=build /src/target/release/mdpdf-app /usr/local/bin/mdpdf-app
COPY --from=build /src/target/release/mdpdf /usr/local/bin/mdpdf
COPY --from=build /src/target/site /app/site
COPY --from=frontend /src/assets /app/licenses/assets
RUN mkdir /app/uploads && chown mdpdf:mdpdf /app/uploads
USER mdpdf
ENV CHROME_PATH=/opt/chromium/chrome MDPDF_HOST=0.0.0.0 MDPDF_PORT=3000 MDPDF_POOL_SIZE=4 MDPDF_ASSET_DIR=/app/uploads LEPTOS_SITE_ROOT=/app/site LEPTOS_OUTPUT_NAME=mdpdf-app LEPTOS_SITE_PKG_DIR=pkg MDPDF_CONTAINER_NO_SANDBOX=1
EXPOSE 3000
VOLUME ["/app/uploads"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s CMD node -e "fetch('http://127.0.0.1:3000/healthz').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["mdpdf-app"]
