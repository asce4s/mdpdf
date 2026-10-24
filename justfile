default:
    @just --list

dev:
    npm run dev

build:
    npm run build

preview:
    npm run preview

test:
    cargo test --workspace --locked
    npm run lint

browser-test:
    npm run test:browser
