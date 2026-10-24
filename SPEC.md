# Leptos Markdown → PDF Tool — Spec

Oct 23, 2026 · @supun

## 1. Overview

**mdpdf** is a full-stack Rust web app built with Leptos that converts Markdown to print-quality PDF, with first-class Mermaid diagrams, a live preview, and a theme panel controlling page layout, fonts, tables, code blocks and diagrams. The core rule: **preview and PDF are produced by the same HTML + CSS + Mermaid renderer**, so what the user sees is what they get.

### Goals

- Paste or open a `.md` file, see a live preview within 300 ms of typing (debounced), export a PDF in one click.
- Render every Mermaid diagram type supported by the pinned Mermaid.js version (flowchart, sequence, class, state, ER, gantt, pie, journey, gitGraph, mindmap, timeline, quadrant, sankey, xychart, block, architecture, etc.) as **vector SVG** in the PDF.
- Expose customization for page, typography, tables, code blocks and diagrams through a typed `Theme` model with presets, import/export, and per-document front-matter overrides.
- Ship a CLI (`mdpdf-cli`) sharing the same core, so conversion is scriptable and testable without the UI.

### Non-goals (v1)

- User accounts, cloud storage, collaboration.
- WYSIWYG rich-text editing (the source of truth is Markdown).
- DOCX/EPUB output.
- Pure-Rust Mermaid rendering (Mermaid.js in a headless browser is the only faithful renderer today).

## 2. Architecture and tech stack

One pipeline, two outputs. Markdown is parsed server-side into a self-contained **print document** (HTML + theme CSS + fonts + vendored Mermaid.js). The browser shows that document in a sandboxed iframe; the server loads the identical document in headless Chromium and calls `Page.printToPDF`.

&#91;embedded content: architecture · one renderer, two outputs\]

Both preview panes and the downloaded file come from `build_print_document()`; only the final step (iframe vs. `printToPDF`) differs.

### Rendering pipeline

1. **Parse** — `comrak` (GFM) → AST. Extract YAML front matter.
2. **Transform** — walk the AST: tag ```` ```mermaid ```` blocks as `<pre class="mermaid" data-diagram-id=…>`, syntax-highlight other code blocks with `syntect` (class-based, theme-driven), resolve page-break markers, build TOC and heading numbers, rewrite image paths to the asset store.
3. **Sanitize** — `ammonia` on any raw HTML in the Markdown, with an allowlist.
4. **Theme** — `Theme` struct → generated CSS (`@page`, CSS custom properties, table/code/heading rules) + a Mermaid `initialize()` config JSON.
5. **Assemble** — one HTML document: theme CSS, `@font-face` rules, vendored `mermaid.min.js`, and a bootstrap script that waits for `document.fonts.ready`, runs `mermaid.run()`, then sets `window.__MDPDF_READY__ = true` (or `__MDPDF_ERRORS__`).
6. **Output** — preview iframe (browser) or `printToPDF` (server, headless Chromium).

### Stack

| Layer | Choice | Notes |
| --- | --- | --- |
| Web framework | Leptos 0.8.x (SSR + hydrate), `leptos_axum` | Latest stable 0.8 line; pin exact version in `Cargo.toml` |
| Build tool | `cargo-leptos` | Workspace with `ssr` / `hydrate` features |
| Server | Axum + Tokio | Server functions for render/export |
| Markdown | `comrak` | GFM tables, task lists, footnotes, strikethrough, autolinks, front matter, header IDs |
| Syntax highlighting | `syntect` | CSS-class output so code themes are swappable |
| HTML sanitizing | `ammonia` | Allowlist-based |
| Diagrams | Mermaid.js 11.x, vendored (no CDN) | Same file used in preview and export |
| PDF | Headless Chromium via `chromiumoxide` | Long-lived browser, page pool; `printToPDF` with vector output |
| Editor | CodeMirror 6 via small JS interop (`wasm-bindgen`) | Markdown mode, line numbers, scroll sync |
| PDF preview | Browser native PDF viewer in `<iframe>` (pdf.js optional fallback) | Shows the real exported PDF |
| Config | `serde` + TOML/JSON | Theme files, front matter |
| CLI | `clap` | Shares `mdpdf-core` and `mdpdf-render` |

Why headless Chromium: it is the only engine that runs Mermaid.js natively and has mature paged-media support (`@page`, page margin boxes, `break-*`, repeating table headers), so preview and PDF stay identical. Typst/pure-Rust PDF stacks would require re-implementing Mermaid layout.

## 3. Markdown support

Target: full CommonMark + GFM, plus a small set of print-oriented extensions.

| Feature | Syntax | Required |
| --- | --- | --- |
| CommonMark core | headings, emphasis, lists, links, images, blockquotes, code | v1 |
| GFM | tables (with alignment), task lists, strikethrough, autolinks, footnotes | v1 |
| Front matter | YAML between `---` lines; keys `title`, `author`, `date`, `theme`, `toc`, `cover`, plus any `Theme` override | v1 |
| Page break | `<!-- pagebreak -->` or a line containing only `\newpage` | v1 |
| Table of contents | `[[toc]]` marker, or `toc: true` in front matter | v1 |
| Heading numbering | Theme option (1, 1.1, 1.1.1) | v1 |
| Admonitions | `> [!NOTE]`, `[!TIP]`, `[!WARNING]`, `[!IMPORTANT]`, `[!CAUTION]` (GitHub style) | v1 |
| Image sizing | `![alt](src){width=50%}` attribute syntax | v1 |
| Figure captions | Image title text or `<!-- caption: … -->` before a block | v1 |
| Raw HTML | Allowed after `ammonia` sanitizing | v1 |
| Math | `$…$` / `$$…$$` via KaTeX (vendored) | v1.1 |
| Cover page | From front matter (`cover: true`) | v1.1 |

Images: local uploads are stored in a per-session asset store and referenced by ID; remote `http(s)` images are fetched server-side with a size cap (10 MB each) and timeout (10 s), then inlined, so export never depends on network access from Chromium.

## 4. Mermaid rendering

Mermaid.js 11.x is vendored into `assets/vendor/mermaid/` and loaded by the print document in both preview and export — never from a CDN. Diagrams stay as inline **SVG** in the PDF (vector, selectable text).

### Behaviour

- Every ```` ```mermaid ```` fence becomes one diagram. Optional fence attributes: ```` ```mermaid {width=80% align=center caption="Login flow" theme=dark} ````.
- Mermaid's own `%%{init: {...}}%%` directive is honoured and overrides the global diagram theme for that diagram only.
- Global config passed to `mermaid.initialize()` comes from `Theme.diagrams` (section 5): `theme`, `themeVariables`, `fontFamily`, `fontSize`, `flowchart.curve`, `sequence.mirrorActors`, `securityLevel: "strict"`, `startOnLoad: false`.
- Fonts: the bootstrap awaits `document.fonts.ready` **before** `mermaid.run()`, so node sizes are measured with the final font (otherwise labels clip in the PDF).
- Sizing: SVGs get `max-width: 100%` of the content box; `width=` attribute scales; diagrams taller than the printable area are scaled down to fit one page (configurable: `fit-page` | `allow-split` | `none`).
- Page breaks: diagram wrappers use `break-inside: avoid`.
- Each diagram is wrapped in `<figure class="mdpdf-diagram">` with optional `<figcaption>` and automatic numbering ("Figure 3") when enabled.

### Errors

- A diagram that fails to parse renders as an error box showing the Mermaid error message and line, with the source in a collapsible block. Other diagrams still render.
- The editor underlines the failing fence (error mapped to editor line numbers).
- Export option `strict_diagrams` (default off): if on, any diagram error fails the export with a 422 listing the errors.

### Performance

- Preview re-renders only diagrams whose source hash changed; unchanged SVGs are reused from a client-side cache keyed by `hash(source + diagram config)`.
- Render timeout per diagram: 5 s; whole document: 30 s.

## 5. Customization (Theme model)

All render options live in one serializable `Theme` struct (in `mdpdf-core`). It compiles to CSS + Mermaid config; nothing is styled ad hoc. Precedence, lowest to highest: built-in defaults → selected preset → user theme edits → document front matter → per-element attributes (e.g. diagram fence attrs).

```rust
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Theme {
    pub name: String,
    pub page: PageOptions,
    pub typography: Typography,
    pub headings: HeadingOptions,
    pub tables: TableOptions,
    pub code: CodeOptions,
    pub diagrams: DiagramOptions,
    pub images: ImageOptions,
    pub header_footer: HeaderFooter,
    pub custom_css: Option<String>, // appended last, sanitized
}
```

### Page

| Option | Values | Default |
| --- | --- | --- |
| `size` | A3, A4, A5, Letter, Legal, Custom(w, h mm) | A4 |
| `orientation` | portrait, landscape | portrait |
| `margins` | top/right/bottom/left in mm | 20/18/20/18 |
| `background` | colour | #ffffff |
| `toc` | on/off, depth 1–4 | off, 3 |
| `heading_numbers` | on/off | off |
| `break_before_h1` | on/off | off |

### Header and footer

Left / centre / right slots for header and footer, each a template string with tokens `{title}`, `{author}`, `{date}`, `{page}`, `{pages}`, `{section}`. Implemented with CSS `@page` margin boxes plus `counter(page)` / `counter(pages)`; fall back to `printToPDF` `headerTemplate`/`footerTemplate` if margin boxes are unsupported in the pinned Chromium. Options: font size, colour, hide on first page.

### Typography

| Option | Values | Default |
| --- | --- | --- |
| `body_font` | bundled font, system font name, or uploaded font ID | Inter |
| `heading_font` | same | Inter |
| `mono_font` | same | JetBrains Mono |
| `base_size` | 8–16 pt | 11 pt |
| `line_height` | 1.0–2.0 | 1.5 |
| `paragraph_spacing` | em | 0.75 |
| `text_color`, `link_color` | colour | #1a1a1a, #0b5fff |
| `text_align` | left, justify | left |
| `hyphenation` | on/off (`hyphens: auto` + `lang` from front matter) | off |

Bundled fonts (OFL, shipped in `assets/fonts/`, WOFF2): Inter, Source Serif 4, Merriweather, IBM Plex Sans, JetBrains Mono, Noto Sans + Noto Sans CJK SC (lazy-loaded). Users can upload TTF/OTF/WOFF2 (≤ 5 MB each); they become `@font-face` rules and are embedded by Chromium into the PDF. Font picker shows a live sample of each font.

### Headings

Per level H1–H6: size (pt), weight, colour, margin top/bottom, optional bottom border, uppercase toggle.

### Tables

| Option | Values | Default |
| --- | --- | --- |
| `style` | grid, horizontal-lines, minimal, none | horizontal-lines |
| `border_color`, `border_width` | colour, 0–3 px | #d0d7de, 1 |
| `header_bg`, `header_color`, `header_weight` | colour, colour, 400–800 | #f6f8fa, inherit, 600 |
| `zebra` | on/off + colour | on, #fafbfc |
| `cell_padding` | vertical/horizontal px | 6/10 |
| `font_size` | relative to body (0.7–1.0) | 0.95 |
| `width` | auto, full | full |
| `repeat_header` | repeat `<thead>` on each page | on |
| `avoid_row_split` | `break-inside: avoid` on rows | on |
| `wide_tables` | wrap, shrink-to-fit, rotate-landscape page | shrink-to-fit |
| `caption_position` | above, below | above |

GFM column alignment (`:---:`) is always honoured.

### Code blocks

Syntax theme (bundled syntect themes: GitHub Light, One Dark, Solarized Light/Dark, Nord, plus `.tmTheme` upload), font size, background, border, line numbers on/off, wrap long lines vs. clip, and `break-inside: avoid` for blocks under N lines (default 25).

### Diagrams

| Option | Values | Default |
| --- | --- | --- |
| `mermaid_theme` | default, neutral, dark, forest, base | neutral |
| `theme_variables` | key/value map passed through (primaryColor, lineColor, etc.) | {} |
| `font_family` | follows body font or explicit | body font |
| `font_size` | px | 14 |
| `max_width` | % of content width | 100 |
| `align` | left, center, right | center |
| `oversize` | fit-page, allow-split, none | fit-page |
| `captions` / `numbering` | on/off | on / off |
| `background` | transparent or colour | transparent |
| `flowchart_curve` | basis, linear, step, cardinal | basis |

When `mermaid_theme = base`, the UI shows colour pickers for the main `themeVariables` (primary, secondary, tertiary, line, text, note background).

### Images

Max width (%), alignment, border radius, optional border/shadow (print-safe), caption style.

### Presets

Ship at least 5 presets as TOML in `assets/themes/`: **Default**, **GitHub**, **Academic** (serif, justified, numbered headings, grid tables), **Corporate** (sans, coloured header row, header/footer), **Compact** (10 pt, tight spacing). Users can save custom themes (browser `localStorage`), and import/export them as `.mdpdf-theme.toml`.

## 6. UI: editor, preview, settings

Single-page Leptos app with a three-pane layout: **Editor | Preview | Settings drawer** (settings collapsible). Panes are resizable; layout switches to tabs below 900 px width. Light/dark UI theme (independent of the document theme).

### Top bar

Open `.md` (file picker + drag-and-drop), New, theme preset dropdown, preview mode toggle, **Export PDF** button (primary), download `.md`, import/export theme.

### Editor pane

- CodeMirror 6 with Markdown highlighting, line numbers, soft wrap, search.
- Inline Mermaid error markers (from section 4).
- Drag-and-drop or paste images → uploaded to asset store → `![](asset://<id>)` inserted at cursor.
- Draft autosaved to `localStorage` every 2 s; restored on reload.
- Snippet menu: insert table, Mermaid templates (flowchart, sequence, ER, gantt, …), page break, TOC.

### Preview pane — two modes

1. **Live (HTML)** — default. The print document rendered in a sandboxed `<iframe srcdoc>` (`sandbox="allow-scripts"`, no same-origin). Updates 300 ms after typing stops. Shows page boundaries as dashed lines at the computed page height so users see where breaks fall (approximate). Scroll-sync with the editor via `data-source-line` attributes emitted per block.
2. **PDF (exact)** — calls the export endpoint and shows the actual PDF in an iframe (`blob:` URL). Refreshed on demand or auto-refreshed 2 s after edits stop (toggle). A spinner and last-render time are shown.

Zoom: 50–200 %; fit width.

### Settings drawer

Accordion sections mirroring `Theme`: Page, Header & Footer, Typography, Headings, Tables, Code, Diagrams, Images, Custom CSS. Every control updates the live preview immediately (no page reload; only CSS + Mermaid config re-injected). Each section has "reset to preset". Font pickers show samples; colour fields use native colour inputs with hex text; Custom CSS uses a small CodeMirror CSS editor.

### State (Leptos)

- `RwSignal<String>` for Markdown source, `RwSignal<Theme>` for theme, `Memo` for compiled CSS.
- Render requests go through a server function `render_preview(md, theme) -> PreviewDoc` with request cancellation (newest wins).
- Theme CSS is compiled in `mdpdf-core`, which must also compile to WASM so theme-only changes re-style the iframe client-side without a server round trip.

## 7. PDF export pipeline and API

### Export flow

1. Server receives `{ markdown, theme, options }`.
2. `mdpdf-core` builds the print document (same function as preview).
3. `mdpdf-render` takes a page from a pool of Chromium tabs (pool size configurable, default 4), with request interception that **blocks all network** except the internal asset route.
4. `page.set_content(html)` → wait for `window.__MDPDF_READY__ === true` (poll, 30 s timeout) → read `window.__MDPDF_ERRORS__`.
5. `Page.printToPDF` with `preferCSSPageSize: true`, `printBackground: true`, `generateTaggedPDF: true`, `generateDocumentOutline: true` (bookmarks from headings).
6. Post-process with `lopdf`: set Title/Author/Subject/Keywords metadata from front matter.
7. Stream back `application/pdf` with `Content-Disposition: attachment; filename="<title>.pdf"`.

Chromium is started once at server boot (`--headless=new`, `--disable-gpu`, `--no-sandbox` only inside the container). If it crashes, the pool restarts it and retries the job once.

### Endpoints / server functions

| Name | Type | Input | Output |
| --- | --- | --- | --- |
| `render_preview` | server fn | markdown, theme | `PreviewDoc { html, diagnostics }` |
| `POST /api/export` | Axum route | `ExportRequest { markdown, theme, filename?, strict_diagrams? }` | `application/pdf` or 422 JSON diagnostics |
| `POST /api/assets` | Axum route | multipart image/font | `{ id, kind, url }` |
| `GET /assets/:id` | Axum route | — | stored file |
| `GET /api/themes` | server fn | — | list of built-in presets |
| `GET /healthz` | Axum route | — | 200 when Chromium pool is ready |

The export route (not a server fn) is used for binary streaming so the browser can download directly.

### Diagnostics type

```rust
pub struct Diagnostic {
    pub severity: Severity,      // Error | Warning
    pub kind: DiagKind,          // Mermaid | Image | FrontMatter | Font | Html
    pub message: String,
    pub source_line: Option<u32>, // line in the Markdown source
}
```

### CLI

`mdpdf input.md -o out.pdf [--theme academic|path.toml] [--set tables.style=grid] [--strict-diagrams] [--html out.html]` — same core and renderer, launches its own Chromium. Exit code 2 on diagram errors in strict mode.

## 8. Project structure, build, security

### Workspace layout

```text
mdpdf/
├── Cargo.toml                 # workspace
├── crates/
│   ├── mdpdf-core/            # md → print document; Theme → CSS; no I/O; compiles to wasm32 + native
│   │   ├── src/{lib,parse,transform,theme,css,frontmatter,toc,diagnostics}.rs
│   │   └── tests/fixtures/    # .md inputs + expected HTML snapshots (insta)
│   ├── mdpdf-render/          # Chromium pool, printToPDF, lopdf metadata (native only)
│   ├── mdpdf-app/             # Leptos app (cargo-leptos), Axum server, UI components
│   │   ├── src/{app,main,lib}.rs
│   │   ├── src/components/{editor,preview,settings,toolbar}/
│   │   └── src/server/{export,assets,themes}.rs
│   └── mdpdf-cli/             # clap CLI
├── assets/
│   ├── vendor/mermaid/        # pinned mermaid.min.js + LICENSE
│   ├── vendor/codemirror/     # bundled CM6 build
│   ├── vendor/katex/          # v1.1
│   ├── fonts/                 # OFL fonts, WOFF2
│   ├── themes/                # preset .toml files
│   └── syntax/                # syntect themes
├── e2e/                       # Playwright tests
├── Dockerfile                 # multi-stage: build + debian-slim with chromium
└── justfile                   # dev, test, e2e, docker
```

### Build and run

- `cargo leptos watch` for dev; `cargo leptos build --release` for prod.
- Rust stable (edition 2024), `wasm32-unknown-unknown` target.
- Chromium: discovered via `CHROME_PATH` env, else `which chromium`/`google-chrome`; Docker image installs `chromium` + `fonts-noto-color-emoji`.
- Config via env: `MDPDF_PORT` (3000), `MDPDF_POOL_SIZE` (4), `MDPDF_MAX_MD_BYTES` (2 MB), `MDPDF_ASSET_DIR`, `MDPDF_ASSET_TTL_HOURS` (24).
- Asset store: local directory, files named by SHA-256; cleanup task deletes files older than TTL.

### Security

- Raw HTML sanitized with `ammonia`; `<script>`, `on*` attributes, `javascript:` URLs stripped.
- Mermaid `securityLevel: "strict"` (no click callbacks, sanitized labels).
- Preview iframe sandboxed without `allow-same-origin`; CSP in the print document allows only inline scripts injected by the server (nonce) and `self` assets.
- Chromium export pages block every network request except `/assets/*`; JavaScript disabled for everything except the bootstrap + Mermaid (enforced by CSP nonce).
- Limits: Markdown 2 MB, 200 diagrams, 50 images, export timeout 30 s, max 2 concurrent exports per IP (simple in-memory rate limiter).
- `custom_css` is size-capped (50 KB) and stripped of `@import` and `url()` to external hosts.

## 9. Testing and acceptance criteria

### Test layers

- **Unit (`mdpdf-core`)** — parser extensions, front matter merge precedence, Theme → CSS generation, TOC, page-break markers, sanitizer. Snapshot tests with `insta` for HTML output of every fixture.
- **Render integration (`mdpdf-render`)** — export each fixture to PDF; assert with `lopdf`/`pdf-extract`: page count, page size, presence of expected text, metadata, outline entries. Mark as `#[ignore]` when Chromium is absent; CI installs it.
- **Visual regression** — rasterize page 1 of each fixture PDF (`pdftoppm`) and compare to golden PNGs with a tolerance (e.g. `image-compare`, ≤ 0.5 % pixel diff).
- **E2E (Playwright)** — type Markdown, see preview update, change table style and see iframe change, click Export and receive a PDF, Mermaid error shows marker.

### Required fixtures (`crates/mdpdf-core/tests/fixtures/`)

`basic.md`, `gfm-tables.md` (alignment, 200-row table spanning pages), `wide-table.md`, `code.md` (10 languages, long lines), `mermaid-all.md` (one of every diagram type), `mermaid-error.md`, `mermaid-init-directive.md`, `fonts-cjk.md`, `images.md`, `frontmatter-overrides.md`, `toc-numbering.md`, `admonitions.md`, `page-breaks.md`, `xss.md` (script tags, event handlers, javascript: links).

### Acceptance criteria

- [ ] Every diagram in `mermaid-all.md` appears in the PDF as vector SVG (text extractable), none clipped.
- [ ] Changing any Theme option in the UI updates the live preview in < 500 ms without a full reload.
- [ ] Live preview and exported PDF match visually for all fixtures (same fonts, colours, table styles, diagram layout).
- [ ] Tables: header row repeats on every page of `gfm-tables.md`; no row is split across pages.
- [ ] Header/footer shows `Page X of Y` correctly on every page.
- [ ] Uploaded custom font is used in body text, headings and Mermaid labels, and is embedded in the PDF.
- [ ] `mermaid-error.md` exports with an error box (non-strict) and returns 422 with diagnostics (strict).
- [ ] `xss.md` produces no executable script in preview or PDF.
- [ ] Export of a 30-page document with 20 diagrams completes in < 10 s on a 4-core machine.
- [ ] PDF has bookmarks from headings and Title/Author metadata from front matter.
- [ ] CLI produces byte-equivalent layout (same page count, same text) as the web export for every fixture.
- [ ] `cargo clippy -- -D warnings`, `cargo fmt --check`, and all tests pass in CI.

## 10. Milestones and instructions for Codex

Build in this order; each milestone must end with green tests before the next starts.

1. **M1 — Core** (`mdpdf-core`): comrak parsing, extensions (page breaks, TOC, admonitions, image attrs, front matter), Mermaid fence tagging, syntect highlighting, sanitizer, `Theme` struct + presets + CSS generation, print-document assembly with bootstrap script. Snapshot tests for all fixtures.
2. **M2 — Renderer + CLI** (`mdpdf-render`, `mdpdf-cli`): Chromium pool, ready-flag wait, network blocking, `printToPDF`, metadata via `lopdf`. CLI end to end. Render integration tests.
3. **M3 — App shell**: Leptos SSR + hydrate, three-pane layout, CodeMirror editor, live HTML preview iframe, `render_preview` server fn, `/api/export` download, autosave.
4. **M4 — Customization UI**: settings drawer for every `Theme` section, client-side CSS recompute (core in WASM), presets, theme import/export, font and image upload, Mermaid `base` theme colour pickers.
5. **M5 — PDF preview + polish**: exact PDF preview mode, scroll sync, page-boundary guides, Mermaid error markers in editor, diagram cache, rate limits, Dockerfile, Playwright E2E, visual regression goldens.
6. **M6 (v1.1)**: KaTeX math, cover page, figure numbering, `.tmTheme` upload.

### Rules for the implementing agent

- Use only crates named in this spec unless one is unmaintained or incompatible; if you substitute, record why in `DECISIONS.md`.
- Pin exact versions of Leptos (0.8.x), Mermaid.js (11.x) and CodeMirror; vendor all JS/CSS/fonts — no runtime CDN.
- The preview and export **must** call the same `build_print_document(md, &theme) -> PrintDocument` function. Never fork rendering logic.
- Every `Theme` field must have a default, a UI control, a CSS/Mermaid mapping, and at least one test.
- Keep `mdpdf-core` free of I/O and `tokio` so it compiles to `wasm32-unknown-unknown`.
- Write a `README.md` with setup (Rust, cargo-leptos, Chromium), dev commands, and a screenshot.
- When something in this spec is ambiguous, choose the simplest option that keeps preview/PDF parity and note it in `DECISIONS.md`.

### Open questions

- Deployment target: single Docker container on a VM, or a serverless runtime? (Chromium favours a long-lived container.)&#32;&#32;
- Should saved themes sync server-side later, or stay browser-only?
