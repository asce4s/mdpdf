# Browser-only delivery status — 2026-10-24

The `browser-only` branch builds a static web application. The server edition remains on `main`. The local static preview is available at **http://127.0.0.1:4173/mdpdf/**. The built directory is `target/browser/` (approximately 24 MB including WASM, fonts, scripts and licenses).

## Implemented

- Client-side Leptos mounting with optimized Rust/WASM parsing, syntax highlighting and a background worker; main-thread WASM fallback when workers are unavailable.
- All live preview preparation in the browser: sanitization, nonce binding, embedded font subsets, Mermaid, KaTeX, images, math and theme compilation. No preview, upload, syntax-import or PDF API calls are used.
- IndexedDB image/font imports with browser-local persistence, content hashes and limits. Relative and reference images can be dropped with Markdown. Remote images use direct credential-free CORS fetches and show import guidance when unavailable.
- Image data is embedded after Markdown parsing, so a large image does not incorrectly consume the 2 MB Markdown budget. Missing asset references produce diagnostics instead of attempted asset API requests.
- Browser Print / Save PDF replaces automatic server PDF downloads and the exact PDF tab. Printing waits for rendering and honors strict diagram errors. Portable HTML embeds scripts, fonts and supported images and renders without network access.
- Persistent sandbox/SVG reuse, cached scrolling and cancellation of obsolete diagrams. Concurrent HTML exports remain separate from coalesced preview jobs. Document links open separately so the live iframe stays on its trusted local runtime.
- Relative paths support origin-root and `/mdpdf/` hosting. The branch includes static build/preview commands, a verification workflow and an opt-in GitHub Pages deployment job. Docker and backend-specific web startup/test recipes have been removed from this branch.

## Verified

- Production static build with the `browser` WASM feature; formatting, workspace/all-target Clippy, explicit WASM browser Clippy, core WASM compilation, JavaScript syntax, two bootstrap tests and all vendor checksums pass.
- All **44 ordinary Rust tests** pass, including local/reference image resolution, unchanged source lines, invalid and missing images, large-image size handling, and shared TextMate parsing. Native browser/HTTP integration suites are retained but are not required or claimed rerun for this static edition.
- All **17 browser tests** pass against a static-only HTTP host. Coverage includes root/project paths, static GET-only boot, opaque sandboxing, drops and SVG reuse, themes/TOML/TextMate, images/fonts across reloads, CORS images, strict printing and actual `beforeprint`, offline HTML, two-page PDF text/page count, cached scrolling, worker fallback, mobile controls, concurrent exports, obsolete diagrams, CJK fonts, images above 2 MB, and external links.
- The offline exported math/cover HTML renders its equation and SVG without external requests. A real browser print produces a two-page PDF with extracted title, body and vector diagram labels.
- All eight supplied Markdown samples render every diagram without errors or iframe navigation. They make **zero backend preview requests**, and 120 cached scroll messages produce **zero geometry reads** for each file.

| Sample | Drop to content update | Fully ready | Diagrams |
| --- | ---: | ---: | ---: |
| `Untitled.md` | 506 ms | 617 ms | 0 |
| `crawler-project-context.md` | 71 ms | 89 ms | 0 |
| `web-crawler-db-design.md` | 568 ms | 4167 ms | 8 |
| `web-crawler-deployment.md` | 263 ms | 312 ms | 0 |
| `web-crawler-high-level-design.md` | 130 ms | 618 ms | 6 |
| `web-crawler-logical-data-model.md` | 286 ms | 759 ms | 3 |
| `web-crawler-low-level-design.md` | 320 ms | 723 ms | 2 |
| `web-crawler-requirements.md` | 230 ms | 282 ms | 0 |

Detailed results are in [docs/browser-performance-report.json](docs/browser-performance-report.json). Timings are single local measurements; complete rendering still depends on document complexity. The database design's eight diagrams take roughly four seconds to finish while text is available sooner.

## Hosting and limits

GitHub Pages has not been published. Pushes verify the branch and upload the static artifact; the optional deployment job runs only when explicitly dispatched with `deploy=true`. Enable Pages with GitHub Actions and permit the branch in the deployment environment before publishing.

The browser controls final PDF pagination, metadata, tags and outlines. There is no embedded exact PDF preview or automatic PDF byte download. Server-specific running `{section}` labels are not implemented by this web edition. Stored imports belong to the current browser/origin and can be removed by clearing site data; storage failures are reported. Remote images require browser access/CORS and can be imported manually instead.

The historical server/container reports remain in `docs/` and describe the `main` edition, not this static build. The workspace's `.git` mount remains read-only, so branch metadata and Git operations use the writable temporary checkout created for publishing.
