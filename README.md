# mdpdf — browser-only

Write Markdown, preview it with Rust/WASM, and save it as PDF using your browser. This branch runs entirely from static files: there is no application server, upload API, or server-side Chromium process.

![The browser-only editor and local preview](docs/screenshot.png)

The [`main` branch](https://github.com/asce4s/mdpdf/tree/main) retains the server-based edition with automatic PDF downloads and exact PDF preview. This `browser-only` branch is suitable for GitHub Pages and other static hosts.

## Build and run

Build tools: Rust 1.91.1+, Node.js 22+, the Rust WASM target and cargo-leptos 0.3.6. A server-side browser is not required to build or run the app.

```sh
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos --version 0.3.6 --locked
npm ci --ignore-scripts
npm run build
npm run preview
```

Open **http://127.0.0.1:4173/**. `npm run dev` builds and starts the same static preview server. After the first setup, `npm run build` creates **`target/browser/`**; publish that directory without a Rust or Node runtime. Node is used only for build tools and the optional local file server. Opening `index.html` through `file://` does not support module workers and WASM reliably; use a static HTTP(S) host.

The output uses relative asset URLs, so the same build works at `/` and at a project path such as `/mdpdf/`. It contains the editor, optimized WASM, Mermaid, KaTeX, fonts and their license notices. It does not use a runtime CDN.

## Use the editor

- Open or drop Markdown, edit it in CodeMirror, and adjust the document theme. Parsing and syntax highlighting run in a background Rust/WASM worker. Browsers without workers fall back to local WASM on the main thread.
- Preview updates reuse the sandbox and unchanged blocks. Scrolling uses cached source positions, and opening another document cancels the remaining diagrams from the previous one.
- Drop Markdown and its PNG/JPEG/WebP images together to resolve relative paths and reference images. You can also choose **Insert → Image** or paste an image. Imported images and fonts stay in IndexedDB in this browser; drafts, themes and preferences use local storage.
- Import TTF, OTF or WOFF2 fonts and TextMate `.tmTheme` syntax styles locally. Theme TOML import/export, presets, structural controls, math, cover pages and figure numbering use the shared Rust renderer.
- Choose **Print / Save PDF**, then select **Save as PDF** in the browser print dialog. Open **More settings** and turn off **Headers and footers** to remove the browser's document name, URL (such as `about:srcdoc`), date/time and page numbers. This browser setting cannot be switched off by the app. Printing waits for fonts and diagrams. Strict diagrams blocks printing while a diagram has an error. The print dialog controls paper settings, scaling and backgrounds.
- Download Markdown or **print HTML** from the document menu. Supported images, used font subsets, Mermaid and KaTeX are embedded into HTML so it can render and print without network access. Markdown exports retain their image references; use HTML for a portable document with imported assets.

Remote Markdown images are fetched directly by your browser, without credentials, and need the image host's CORS permission. If a remote image cannot be read, import its local file instead. No server proxy is involved. Imported files are local to the current browser and site origin; clearing site data removes them, and private browsing or storage quotas can prevent persistence. The editor reports storage failures and keeps the current file in memory when possible.

Limits: 2 MB of Markdown, 50 document images, 10 MB per image, 5 MB per font, 100 imported files including 10 fonts, and 64 MB of imported assets. Bundled fonts are fetched from the static host as needed. Fonts and runtime resources already loaded in the current tab are reused.

Final pagination is provided by your browser's print preview. This edition does not generate PDF bytes for an embedded exact-preview tab. Server-specific bookmark/tag/metadata processing and running `{section}` labels are not supplied by the web app; output capabilities depend on the browser. The native CLI and renderer sources remain in the workspace for the server edition, but they are not needed to serve `target/browser/`.

## GitHub Pages

The **Verify browser app** workflow builds and tests this branch on pushes and uploads a `browser-site` artifact. Pushing the branch does not publish the site automatically.

To publish when ready:

1. In the repository's **Settings → Pages**, set **Source** to **GitHub Actions**.
2. Allow `browser-only` to deploy to the `github-pages` environment if that environment restricts deployment branches.
3. Dispatch the existing workflow from this branch with publishing enabled:

   ```sh
   gh workflow run ci.yml --repo asce4s/mdpdf --ref browser-only -f deploy=true
   ```

The workflow runs all checks before its optional Pages deployment job. The project URL will be **https://asce4s.github.io/mdpdf/**. GitHub's [custom Pages workflow documentation](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages) describes the repository and deployment requirements.

Other static hosts can publish `target/browser/` directly after running `npm run build`. No API URL, runtime environment variables, Docker container or secrets are needed by the app.

## Verify

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p mdpdf-core --target wasm32-unknown-unknown --locked
npm run lint
node --test crates/mdpdf-core/tests/bootstrap.test.cjs
python3 scripts/check_assets.py
npx playwright install --with-deps chromium
npm run test:browser
```

The browser suite serves only static files, tests both root and `/mdpdf/` hosting, and checks that the app makes no backend requests. It covers local images/fonts, reload persistence, theme and syntax imports, math, cover pages, print readiness, strict errors, offline HTML export, a real PDF print, scroll indexing and worker fallback. Poppler (`pdfinfo` and `pdftotext`) is needed for PDF verification, and Playwright's Chromium is a test dependency only.

To measure your own Markdown samples against the static preview:

```sh
node scripts/serve_browser.mjs --base /mdpdf/
# In another terminal:
node scripts/benchmark_preview.mjs markdown
```

Results are written to `target/verification/browser-performance.json`. The `markdown/` folder is a local test-input folder and is excluded from Git. See [PROGRESS.md](PROGRESS.md) for recorded verification and [DECISIONS.md](DECISIONS.md) for design history.
