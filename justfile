dev:
    npm run dev

build:
    npm run vendor
    cargo leptos build --release
    cargo build --release -p mdpdf-cli

test:
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    node --test crates/mdpdf-core/tests/bootstrap.test.cjs
    python3 scripts/check_assets.py

browser-test:
    cargo test --workspace -- --ignored --nocapture
    npm run test:e2e

visual:
    node scripts/verify_delivery.mjs

docker:
    docker build -t mdpdf .

run-container:
    docker run --rm -p 3000:3000 --shm-size=1g -v mdpdf-assets:/app/uploads mdpdf

desktop:
    cargo leptos build
    cargo run -p mdpdf-desktop
