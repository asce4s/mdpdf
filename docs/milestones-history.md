# mdpdf progress

M1 and M2 implementation are verified. Milestone commits remain blocked by the empty, read-only `.git` mount. M3–M5 have not started.

## M1 work

- [x] Read the complete spec before code changes.
- [x] Create the four Cargo workspace members and the section 8 directories.
- [x] Add typed Theme sections, defaults, five TOML presets, recursive theme patches, CSS generation, and Mermaid configuration.
- [x] Add front-matter parsing, diagnostics and tests for merge precedence/CRLF/malformed input.
- [x] Write the shared `build_print_document` pipeline: comrak AST transforms, heading IDs/TOC/numbering, page markers, image attributes/captions, Mermaid fences, syntect classes, ammonia sanitizing, source-line attributes, and document assembly. Compiled and verified by pipeline tests and Chromium.
- [x] Write font CSS and the font-ready Mermaid bootstrap with strict configuration, timeouts and isolated diagram error boxes.
- [x] Create every named fixture, including the 200-row table, ten code languages, Mermaid families, CJK and XSS cases.
- [x] Write insta HTML snapshot tests for all 14 fixtures, pipeline tests and per-field Theme mapping tests.
- [x] Vendor Mermaid 11.17.2, seven variable font families with normal/available italic subsets, and complete syntax themes with MIT/OFL notices. Record immutable sources, npm integrity and individual SHA-256 checksums; all 197 pinned files verify.
- [x] Verify Mermaid coverage against the actual pinned build. Fix quoted radar labels and add eleven examples for newly supported families/grammar variants. All 34 examples produce SVG in Chromium.
- [x] Add pure `with_embedded_assets` delivery binding for font/Mermaid data URLs. Chromium verifies loaded fonts and SVG rendering inside opaque `sandbox="allow-scripts"` srcdoc iframes. Force diagram-font loading before measurement, and disable Mermaid autorun before yielding to avoid duplicate/racing renders. PDF sizing/clipping validation remains in M2.
- [x] Generate and inspect all 14 insta HTML snapshots from the real comrak/syntect/ammonia pipeline. Snapshot tests pass without update mode. Snapshot files await the blocked milestone commit.
- [x] Pass formatting, all 30 Rust tests, and Clippy with warnings denied, including native examples.
- [x] Compile core for `wasm32-unknown-unknown` and inspect its normal dependency tree. Core invokes no I/O and has no Tokio, Oniguruma or native Chromium dependencies. Syntect retains dormant filesystem APIs/transitive walkdir; production features exclude plist/YAML loaders, with plist loading enabled only for the development syntax generator.
- [ ] Create the M1 commit after all checks pass and writable Git metadata is available.

## Verification (2026-10-23 retry)

- Dependency downloads succeeded after approved Cargo cache access; `Cargo.lock` is generated.
- `cargo fmt --check`: **passed**.
- `cargo clippy --all-targets --offline -- -D warnings`: **passed**.
- `cargo test --workspace --offline`: **passed**, 30 tests (3 front matter, 22 pipeline/snapshot, 5 theme contract).
- `cargo check -p mdpdf-core --target wasm32-unknown-unknown --offline`: **passed** after installing the target.
- `node --test crates/mdpdf-core/tests/bootstrap.test.cjs`: **passed**, 2 tests including the autorun/font-order regression.
- `python3 scripts/check_assets.py`: **passed**, 197 vendor files plus every font CSS reference.
- `node scripts/check_browser.cjs`: **passed** using real Chromium, core-built embedded documents and opaque sandboxed srcdoc iframes. All 34 Mermaid examples render; the intentionally broken diagram reports one error while its neighbour renders; directives/CJK/XSS fixtures pass. These checks do not assert PDF pagination or visual goldens.
- Generated fixture HTML snapshots and complete syntax CSS were reviewed. Upstream licenses and package declarations were checked; legal review is not claimed.
- `git status --short`: **failed**, because the read-only `.git` mount contains no repository metadata. No milestone commit exists. No replacement Git repository was initialized.
- At the M1 checkpoint the application and CLI were placeholders; the CLI is now implemented as part of M2 below.

## M2 renderer and CLI

- [x] Add chromiumoxide 0.9.1, lopdf 0.46.0, Tokio 1.54.1 and clap 4.6.7 with pinned native support dependencies.
- [x] Embed Mermaid and every declared bundled font into the native binary at build time; share one encoded asset map. Optimize core font embedding to assemble CSS in a single pass.
- [x] Implement one Chromium process with reusable worker tabs, a bounded queue, configurable pool size (default four), 30-second export deadlines, and explicit shutdown.
- [x] Block all external requests, poll the shared ready flag, collect diagram diagnostics, and reject diagram errors in strict mode.
- [x] Retry a browser failure once, coordinate restarts across workers by generation, and discard timed-out tabs.
- [x] Print CSS-sized/background/tagged PDFs with heading outlines; write UTF-16BE Title/Author/Subject/Keywords while preserving Chromium metadata.
- [x] Implement CLI presets/theme files, repeated typed dotted overrides, output paths, optional identical HTML, diagnostics and strict exit code 2. Prevent input/output aliases and preserve existing files on strict failure.
- [x] Export all 14 fixtures in Chromium. Assert page sizes, PDF tags/outlines, metadata, CJK text/embedded glyphs, page breaks, repeating table headers, intact table rows, page counters, vector Mermaid output, and strict/non-strict diagram behavior.
- [x] Exercise concurrent jobs, post-error reuse, blocked HTTP probes, forced browser crashes, timeout recovery and shutdown. Compare actual CLI page count/text against library output.
- [x] Fix Mermaid timeline rasterization by disabling SVG filters in shared CSS. Normalize measured SVG bounds for Gantt, XY and tree-view clipping in the common bootstrap. Browser checks now report zero out-of-viewBox diagrams or rasterizing effects across all 34 examples.
- [x] Regenerate/review fixture snapshots after the shared CSS/bootstrap changes; visually inspect rasterized CJK and Mermaid pages.
- [ ] Create milestone commits once real, writable Git metadata is available.

M2 verification commands:

- `cargo fmt --check`, `cargo clippy --all-targets --offline -- -D warnings`, and `cargo test --workspace --offline`: pass (32 ordinary Rust tests; three external-program tests are skipped by default).
- `cargo test -p mdpdf-render -p mdpdf-cli --offline -- --ignored --nocapture`: **passed** all three Chromium/Poppler suites (fixture/PDF contracts, lifecycle/network isolation, CLI parity and strict-file preservation).
- `cargo check -p mdpdf-core --target wasm32-unknown-unknown --offline`: passes with native renderer dependencies excluded from core.
- Bootstrap tests (2), the 197-file asset checksum check, and the sandboxed Chromium check pass.
- Actual CLI smoke exports produced tagged, selectable A4 PDFs and matching self-contained HTML. `pdffonts` confirms embedded Unicode CJK subsets; Chromium 155 uses Type 3 vector glyph programs for those fonts.

Remaining delivery limits: no web UI/HTTP endpoints yet; uploads and local/remote Markdown image resolution still need the shared asset-store path. Existing embedded PNG/JPEG/WebP data URLs export. The image fixture includes an unresolved upload ID and an intentionally unavailable remote image; its PDF test verifies export, not upload resolution. Full visual goldens and the performance benchmark remain M5 work.

## Section 9 acceptance checklist

- [x] All 34 Mermaid examples render as SVG; their shared-bootstrap view boxes contain the measured bounds. The PDF has extractable labels and no raster image XObjects. Full visual goldens remain M5.
- [ ] Every Theme UI option updates the live preview in under 500 ms without reload. UI deferred to M3/M4.
- [ ] Live preview and PDF visually match all fixtures. Needs both delivery paths and visual regression tests.
- [x] Multipage table headers repeat and rows never split. Verified across all 200 rows and eight exported pages.
- [x] Footer shows correct `Page X of Y` on all three pages in the renderer test.
- [ ] Uploaded custom fonts apply to body/headings/diagrams and are embedded in PDF. Uploads and font embedding unimplemented.
- [ ] Mermaid failures export as boxes normally and return HTTP 422 in strict mode. Renderer error boxes and strict errors, plus CLI exit code 2, are verified. HTTP 422 mapping remains M3.
- [x] Sanitized XSS fixture passes opaque-iframe Chromium checks and PDF export; the renderer also blocks an HTTP probe when test HTML deliberately bypasses CSP.
- [ ] Thirty pages with twenty diagrams export in under ten seconds on four cores. No benchmark yet.
- [x] PDF contains heading bookmarks and Title/Author/Subject/Keywords metadata, verified from actual exports.
- [ ] CLI and web export have identical page count and text for every fixture. M2/M3 pending.
- [ ] Formatting, Clippy with warnings denied, and all tests pass in CI. Local formatting, Clippy and tests pass; CI is scheduled for later delivery work.

## Remaining delivery milestones

- [x] M2: Chromium pool, interception, ready-flag wait, tagged PDF/outline, metadata, CLI and integration tests. Git commit remains unavailable.
- [ ] M3: Leptos SSR/hydrate, CodeMirror, three-pane shell, sandboxed preview, shared render endpoint, download and autosave.
- [ ] M4: controls for every Theme field, theme persistence/import/export, fonts/images and Mermaid colour controls.
- [ ] M5: exact PDF preview, scroll sync, guides, error markers, diagram cache, rate limits, Dockerfile, Playwright, visual goldens, screenshot.
