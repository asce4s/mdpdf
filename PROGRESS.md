# Delivery status — 2026-10-24

Implementation covers M1–M6 from `SPEC.md`. The updated native production application is running for review at **http://127.0.0.1:3100** with optimized browser WASM. The Docker verification below records the October 23 image; the container has not been rebuilt for the October 24 performance changes. The original milestone log is preserved in [docs/milestones-history.md](docs/milestones-history.md).

## Completed implementation

- **M1:** pure native/WASM core, CommonMark/GFM extensions, safe raw HTML, typed Theme/presets, source locations, complete syntax themes, vendored Mermaid/fonts and fixture snapshots.
- **M2:** one Chromium process with a bounded reusable tab pool, request isolation, deadlines, crash recovery, tagged/vector PDFs, bookmarks/metadata and a working CLI. Page counters and running section labels use actual printed pagination.
- **M3:** Leptos SSR/hydration, real signals/memo and preview server function, CodeMirror, resizable editor/preview/settings panes, autosave, responsive tabs and PDF/Markdown/HTML download.
- **M4:** controls for every Theme leaf, local WASM styling and structural updates, presets, per-section reset, saved themes, TOML import/export, system/bundled/uploaded font selection and samples, uploaded/remote/local images, Mermaid palettes and a CSS editor.
- **M5:** exact PDF mode and auto-refresh, zoom/fit, approximate page guides, bidirectional scroll sync, source diagnostics/CodeMirror markers, trusted diagram cache, deterministic diagram IDs/automatic Git commit labels, two concurrent exports per IP, session limits/TTL cleanup, Dockerfile, justfile, CI workflow, E2E and visual goldens.
- **M6:** local KaTeX math with its fonts/license, front-matter cover pages, image/diagram numbering, scoped `.tmTheme` import and portable syntax styles. A dedicated math/cover fixture verifies both its cover and equation page.

## Verified locally

- Production app + optimized WASM build with cargo-leptos 0.3.6; release CLI build.
- `cargo fmt --check`; workspace/all-target Clippy with warnings denied.
- All **39 ordinary Rust tests**, including parser/snapshots, every Theme leaf, SSR asset cleanup ownership, font subsets, image isolation and syntax import.
- Four opt-in Chromium/HTTP suites: all 15 fixture exports and PDF contracts; browser lifecycle/crash/timeout/network isolation; CLI parity/strict-file preservation; real HTTP preview/export, session-scoped uploads, custom font embedding, strict HTTP 422 and concurrent HTTP 429.
- Eight Playwright tests against production: hydration without console errors, local theme changes under 500 ms in the tested case, editing/autosave, every Theme control, downloads/exact PDF, source diagnostics, image/font upload and local font switching, mobile tabs/color theme, math styles/cover/syntax upload, scroll synchronization and local TOC/numbering/preset changes.
- Core builds for `wasm32-unknown-unknown`; native renderer/Tokio stay outside core.
- Two bootstrap ordering/error-isolation tests and checksum verification of 197 original vendor files, 23 KaTeX files and the CodeMirror bundle.
- Opaque sandbox checks: all 34 Mermaid families have unique IDs, zero clipped view boxes and zero rasterizing filters; directives, CJK, XSS, failure isolation and KaTeX pass.
- CLI/web exports have the same page counts and extracted text across all 15 fixtures. The required 200-row table spans eight pages with repeated headers and unsplit rows; the Mermaid fixture spans 20 pages with selectable vector labels.
- All **16 PNG baselines pass without update mode**: page one of each fixture plus page two of math/cover. Cairo rasterization avoids the glyph omissions observed with Poppler Splash. The baselines were visually inspected. Pixel tolerance is 0.5%, counting RGB channel differences above 16.
- Four-core-constrained production server/Chromium benchmark: **30 pages, 20 diagrams in 2.29 seconds** (10-second target). The latest machine-readable report is `target/verification/report.json`.
- Updated screenshot: [docs/screenshot.png](docs/screenshot.png). Setup, environment, usage, tests and container commands are documented in `README.md`.
- **Docker follow-up:** clean `docker build -t mdpdf .` and container startup pass with Docker 29.9.1. Container `mdpdf-container-check` runs as UID/GID 10001 with Chromium 156.0.8078.4 and reports healthy. All eight Playwright tests pass against its HTTP endpoint, including uploads, PDF downloads and strict diagram errors. All 15 fixture exports match the native CLI's page counts/text and all 16 PNG baselines pass without updates. The container's 30-page/20-diagram workload takes **1.62 seconds** (not constrained to four cores). Its bundled CLI also exports the math/cover fixture as a two-page tagged PDF with metadata and selectable equations. Results and image identity are preserved in [docs/container-verification-report.json](docs/container-verification-report.json).
- The follow-up also rechecked all 39 ordinary Rust tests, JavaScript syntax, bootstrap tests and vendor checksums, and reran all eight browser tests and 16 PNG comparisons against the native production server; all pass.

## Preview performance follow-up — 2026-10-24

- Markdown parsing and syntax highlighting run in a Rust/WASM module worker. Both development and production select the optimized browser profile. Syntect definitions initialize lazily once per instance, and theme compilation avoids full parsing just to retain math CSS.
- Live updates reuse the opaque iframe, embedded fonts and unchanged blocks/SVGs. Native updates supply image assets or newly needed font subsets without retransmitting Mermaid. Text can appear while fonts load; diagrams wait for those fonts. Opening a newer file cancels the remaining obsolete diagrams after the current Mermaid call finishes.
- File opens/drops bypass the typing debounce. Capture-phase file handling prevents CodeMirror from processing a drop twice. Scroll updates are coalesced and binary-search cached positions instead of reading every element's geometry.
- All **16 Playwright tests pass**, including iframe/SVG reuse without preview requests, real file drops, cached scrolling, shorter-document sizing, stale file/theme requests, worker fallback, early text while fonts are delayed, and cancellation of old diagrams. The follow-up also passed all 39 ordinary Rust tests, four Chromium/HTTP integration suites, formatting, Clippy, WASM compilation, bootstrap tests and vendor verification.
- CLI/web text and page counts match for all 15 PDF fixtures; all 16 existing PNG baselines pass without updates. The 30-page/20-diagram export took 2.12 seconds during concurrent checks. The standalone sandbox check also passed all 34 Mermaid families, CJK and math.
- All eight supplied `markdown/` documents preserve their source after an actual drop and render every diagram with no errors or iframe navigation. Each recorded **zero geometry reads for 120 scroll messages** after layout settled. The earlier full-reload path took 1.5–5.6 seconds to reach complete readiness. The table separates the updated preview's first content message from complete font/diagram readiness; these are single local measurements, and content messages are not browser paint measurements.

| Sample | Drop to content update | Fully ready | Diagrams |
| --- | ---: | ---: | ---: |
| `Untitled.md` | 456 ms | 933 ms | 0 |
| `crawler-project-context.md` | 30 ms | 41 ms | 0 |
| `web-crawler-db-design.md` | 501 ms | 5001 ms | 8 |
| `web-crawler-deployment.md` | 193 ms | 427 ms | 0 |
| `web-crawler-high-level-design.md` | 59 ms | 596 ms | 6 |
| `web-crawler-logical-data-model.md` | 192 ms | 970 ms | 3 |
| `web-crawler-low-level-design.md` | 198 ms | 909 ms | 2 |
| `web-crawler-requirements.md` | 74 ms | 412 ms | 0 |

The eight-diagram database design still takes about five seconds to finish Mermaid layout, while its text updates in about half a second and the editor stays available. Detailed measurements and baseline methodology are in [docs/preview-performance-report.json](docs/preview-performance-report.json); rerun `MDPDF_BASE_URL=http://127.0.0.1:3100 node scripts/benchmark_preview.mjs markdown` to refresh `target/verification/preview-performance.json`.

## Environment limits

- CI is configured, but this local session cannot claim a hosted CI run.
- The workspace has no usable Git metadata. Milestone commits cannot be created; no alternate repository was initialized.

The live HTML view is intentionally an approximation of pagination. Exact PDF mode uses the real exported bytes. Saved themes/drafts are browser-local, and uploads belong to the current cookie session and expire according to the configured TTL. Running `{section}` labels use top-level PDF outline headings. Figure numbering uses separate image and diagram sequences.
