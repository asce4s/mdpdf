use leptos::prelude::*;

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <meta name="theme-color" content="#171b19"/>
                <title>"mdpdf — A better page for your Markdown"</title>
                <link rel="icon" type="image/svg+xml" href="/favicon.svg"/>
                <link rel="stylesheet" href="/app.css"/>
                <script type="module" src="/app.js"></script>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
            </head>
            <body><App/></body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    #[cfg(all(feature = "hydrate", target_arch = "wasm32"))]
    crate::client::install_bridge();
    view! {
        <div id="workspace" data-mobile-pane="editor">
            <header class="topbar">
                <a class="brand" href="/" aria-label="mdpdf home"><span class="brand-mark">"m"<span>"↓"</span></span><span>"mdpdf"</span><span class="version-label">"WORKSPACE"</span></a>
                <div class="file-identity"><span class="file-icon">"▤"</span><input id="filename" aria-label="Document filename" value="project-brief.md"/><span id="save-state" class="save-state">"Loading…"</span></div>
                <nav class="top-actions" aria-label="Document actions">
                    <button id="open-file" class="button subtle"><span>"↥"</span>" Open"</button>
                    <button id="document-menu" class="icon-button" aria-label="More document actions">"•••"</button>
                    <span class="top-divider"></span>
                    <button id="ui-theme" class="icon-button" aria-label="Toggle interface color theme">"☼"</button>
                    <button id="export-pdf" class="button primary"><span>"↓"</span>" Export PDF"</button>
                </nav>
            </header>
            <div class="workspace-toolbar">
                <div class="workspace-breadcrumb"><span class="status-dot"></span>"Your words. Beautifully printed."</div>
                <div class="workspace-options"><label for="preset">"Document theme"</label><select id="preset"><option value="default">"Default"</option><option value="github">"GitHub"</option><option value="academic">"Academic"</option><option value="corporate">"Corporate"</option><option value="compact">"Compact"</option></select><button id="toggle-settings" class="button subtle"><span>"☷"</span>" Settings"</button></div>
            </div>
            <nav class="mobile-tabs" aria-label="Workspace panes"><button data-pane="editor" class="active">"Editor"</button><button data-pane="preview">"Preview"</button><button data-pane="settings">"Settings"</button></nav>
            <main class="panes">
                <section class="editor-pane" aria-label="Markdown editor">
                    <div class="pane-toolbar"><div class="pane-label"><span class="code-icon">"⌘"</span>"MARKDOWN"</div><div class="editor-tools"><button id="write-mode" class="zen-entry" aria-label="Zen writing mode">"Zen"</button><button id="search-document" class="icon-button" aria-label="Search document">"⌕"</button><select id="insert-snippet" aria-label="Insert snippet"><option value="">"+ Insert"</option><option value="table">"Table"</option><option value="flowchart">"Flowchart"</option><option value="sequence">"Sequence diagram"</option><option value="er">"Entity relationship"</option><option value="gantt">"Gantt chart"</option><option value="pagebreak">"Page break"</option><option value="toc">"Table of contents"</option><option value="image">"Image…"</option></select></div></div>
                    <div id="editor" aria-label="Markdown source"></div>
                    <div class="editor-bottom"><span id="document-stats">"Markdown"</span><span>"UTF-8"<span class="bottom-dot">"·"</span>"Ln "<span id="cursor-line">"1"</span></span></div>
                </section>
                <div id="editor-resizer" class="pane-resizer" role="separator" aria-label="Resize editor" aria-orientation="vertical" tabindex="0"></div>
                <section class="preview-pane" aria-label="Document preview">
                    <div class="pane-toolbar preview-toolbar"><div class="segmented" role="tablist" aria-label="Preview mode"><button id="live-mode" class="selected" role="tab" aria-selected="true">"Live preview"</button><button id="pdf-mode" role="tab" aria-selected="false">"PDF"<span class="exact-tag">"EXACT"</span></button></div><div class="preview-actions"><button id="read-mode" class="zen-entry" aria-label="Zen reading mode">"Zen"</button><button id="refresh-pdf" class="icon-button hidden" aria-label="Refresh PDF">"↻"</button><button id="toggle-guides" class="icon-button enabled" aria-label="Toggle page guides" aria-pressed="true">"▱"</button><button id="toggle-sync" class="icon-button enabled" aria-label="Toggle scroll sync" aria-pressed="true">"⇅"</button><span class="small-divider"></span><button id="zoom-out" class="icon-button" aria-label="Zoom out">"−"</button><select id="zoom" aria-label="Preview zoom"><option value="fit">"Fit width"</option><option value="0.5">"50%"</option><option value="0.75">"75%"</option><option value="1">"100%"</option><option value="1.25">"125%"</option><option value="1.5">"150%"</option><option value="2">"200%"</option></select><button id="zoom-in" class="icon-button" aria-label="Zoom in">"+"</button></div></div>
                    <div id="preview-scroll" class="preview-scroll"><div id="paper-stage"><iframe id="live-preview" title="Live print document" sandbox="allow-scripts"></iframe></div><iframe id="pdf-preview" title="Exact PDF preview" class="hidden"></iframe><div id="preview-empty" class="preview-empty"><span class="preview-empty-mark">"m↓"</span><p>"Preparing your page…"</p></div></div>
                    <div class="preview-bottom"><span id="render-status"><span class="status-dot"></span>"Starting renderer"</span><span id="page-status">"A4 · Portrait"</span></div>
                </section>
                <div id="settings-resizer" class="pane-resizer" role="separator" aria-label="Resize settings" aria-orientation="vertical" tabindex="0"></div>
                <aside class="settings-pane" aria-label="Document settings">
                    <div class="pane-toolbar"><div class="pane-label">"DOCUMENT SETTINGS"</div><button id="reset-theme" class="icon-button" aria-label="Reset theme to preset">"↺"</button></div>
                    <div class="settings-intro"><span class="eyebrow">"MAKE IT YOURS"</span><h1>"A little polish."</h1><p>"Fine-tune the page, from the first heading to the final footnote."</p></div>
                    <div id="settings-controls"></div>
                    <div class="theme-management"><button id="save-theme" class="button outlined">"+ Save as a theme"</button><div><button id="import-theme" class="text-button">"Import"</button><span>" / "</span><button id="export-theme" class="text-button">"Export theme"</button></div></div>
                    <details class="settings-section advanced-section"><summary>"Export options"</summary><div class="settings-section-body"><label class="switch-row"><span>"Strict diagrams"</span><input id="strict-diagrams" type="checkbox" role="switch"/></label><p class="field-help">"Stop export if a diagram needs attention."</p><label class="switch-row"><span>"Auto-refresh exact PDF"</span><input id="auto-pdf" type="checkbox" role="switch"/></label><button id="upload-font" class="button outlined">"↥ Upload a font"</button><p class="field-help">"TTF, OTF or WOFF2 · Up to 5 MB"</p></div></details>
                    <div class="settings-footer"><span class="status-dot"></span>"One document. One shared renderer."</div>
                </aside>
            </main>
            <div id="diagnostics" class="diagnostics hidden" role="status"></div>
            <div id="toast" class="toast hidden" role="status" aria-live="polite"></div>
            <div id="document-actions" class="popover hidden"><button id="new-document">"New document"</button><button id="download-md">"Download Markdown"</button><button id="download-html">"Download print HTML"</button></div>
            <input id="markdown-file" type="file" accept=".md,.markdown,.txt" hidden/>
            <input id="theme-file" type="file" accept=".toml" hidden/>
            <input id="image-file" type="file" accept="image/png,image/jpeg,image/webp" hidden/>
            <input id="font-file" type="file" accept=".ttf,.otf,.woff2" hidden/>
            <nav id="zen-controls" aria-label="Zen mode">
                <button id="zen-read" aria-label="Read" title="Read (Alt+1)" aria-keyshortcuts="Alt+1" aria-pressed="false"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 5v15M12 5C9 3 5 3 2 4v15c3-1 7-1 10 1 3-2 7-2 10-1V4c-3-1-7-1-10 1Z"/></svg></button>
                <button id="zen-write" aria-label="Write" title="Write (Alt+2)" aria-keyshortcuts="Alt+2" aria-pressed="false"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m16 3 5 5M3 21l5-1L21 7a2 2 0 0 0-5-5L3 15v6Z"/></svg></button>
                <button id="zen-find" aria-label="Find in rendered document" title="Find (Ctrl/Cmd+F)"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" aria-hidden="true"><circle cx="10" cy="10" r="6"/><path d="m15 15 6 6"/></svg></button>
                <button id="exit-read" aria-label="Exit zen mode" title="Exit zen (Esc)" aria-keyshortcuts="Escape"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg></button>
            </nav>
        </div>
    }
}
