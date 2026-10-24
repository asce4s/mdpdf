# Asset inventory

Vendored for M1:

- Mermaid **11.17.2**, its MIT license and preserved bundled dependency notices.
- Inter **5.3.0**, Source Serif 4 **5.3.1**, Merriweather **5.3.0**, IBM Plex Sans **5.3.0**, JetBrains Mono **5.3.0**, Noto Sans **5.3.0** and Noto Sans SC **5.3.1**, through pinned `@fontsource-variable` packages. All include upstream OFL licenses. Normal and available italic variable fonts retain their Unicode subsets. SC subsets are fetched only when their glyphs are used when serving fonts; embedding includes the bytes for offline sandboxed documents.
- GitHub Light, One Dark, Solarized Light/Dark and Nord complete syntax sources, with their MIT license notices. Sources are pinned to immutable upstream Git revisions. Nord's Sublime format is converted into a TextMate theme; generated CSS is scoped to `.mdpdf-code`. The selected syntax theme controls token colours while `Theme.code.background` controls the code block background.
- Five theme TOML presets and the repository-owned Mermaid bootstrap.

`vendor-manifest.json` records npm integrity values and individual file SHA-256 hashes. `syntax/manifest.json` records immutable source URLs and hashes. The vendoring scripts verify npm integrity, WOFF2 signatures and font license declarations; no application runtime downloads these assets.

Verify local bytes without downloading:

```sh
python3 scripts/check_assets.py
```

Reproduce the vendored assets (requires setup-time network access):

```sh
python3 scripts/vendor_assets.py
python3 scripts/vendor_syntax.py
cargo run -p mdpdf-core --example syntax_css
```

The native `fixture_html` example loads delivery-side assets supplied by `scripts/check_browser.cjs`. Core itself performs no filesystem or network I/O. `PrintDocument::with_embedded_assets` converts font references and the Mermaid script to typed data URLs, allowing an opaque `sandbox="allow-scripts"` iframe to render them. The delivery layer still supplies the fresh CSP nonce.

Pending: the CodeMirror **6.0.2** editor bundle and exact dependency lockfile in M3; KaTeX in v1.1. No empty binaries or CDN fallbacks stand in for these assets.
