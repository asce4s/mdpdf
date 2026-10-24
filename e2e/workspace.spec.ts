import { test, expect } from '@playwright/test';
import path from 'node:path';
import { readFileSync } from 'node:fs';

async function ready(page) { await page.goto('/'); await page.waitForFunction(()=>window['mdpdf']?.ready); }
async function replace(page, text:string) { const source=page.locator('#editor .cm-content');await source.click();await page.keyboard.press('ControlOrMeta+a');await page.keyboard.insertText(text); }

test('SSR hydrates, editing updates the sandbox, and theme-only changes stay local',async({page})=>{
  const errors:string[]=[];page.on('pageerror',error=>errors.push(error.message));
  let requests=0;page.on('request',request=>{if(request.url().includes('/api/rpc/render_preview'))requests++;});
  await ready(page);
  expect(errors).toEqual([]);
  await expect(page.locator('#live-preview')).toHaveAttribute('sandbox','allow-scripts');
  const preview=page.frameLocator('#live-preview');
  await expect(preview.locator('h1')).toContainText('A better way');
  const id=await preview.locator('svg').first().getAttribute('id');
  await page.locator('summary').filter({hasText:/^Typography$/}).click();
  const before=requests;
  const started=await page.evaluate(()=>{const input=document.querySelector('[data-theme-path="typography.text_color"]') as HTMLInputElement;input.value='#314a27';const start=performance.now();input.dispatchEvent(new Event('input',{bubbles:true}));return start;});
  await expect(preview.locator('html')).toHaveCSS('color','rgb(49, 74, 39)');
  const elapsed=await page.evaluate(start=>performance.now()-start,started);
  expect(elapsed).toBeLessThan(500);
  expect(requests).toBe(before);
  expect(await preview.locator('svg').first().getAttribute('id')).toBe(id);
  await replace(page,'# A fresh page\n\nA live document.\n\n| A | B |\n| --- | --- |\n| One | Two |');
  await expect(preview.locator('h1')).toHaveText('A fresh page');
  await page.locator('summary').filter({hasText:/^Tables$/}).click();
  await page.locator('[data-theme-path="tables.style"]').selectOption('grid');
  await expect(preview.locator('td').first()).toHaveCSS('border-top-style','solid');
  await page.locator('[data-theme-path="tables.font_size"]').fill('0.8');
  await expect(preview.locator('table')).toHaveCSS('font-size','11.7333px');
  await page.waitForTimeout(2200);await page.reload();await page.waitForFunction(()=>window['mdpdf']?.ready);
  await expect(page.locator('#editor .cm-content')).toContainText('A fresh page');
  expect(errors).toEqual([]);
});

test('all Theme leaves have controls and custom page dimensions are editable',async({page})=>{
  await ready(page);
  const missing=await page.evaluate(()=>{
    const paths:string[]=[];
    function walk(value,path=''){if(value!==null&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).length===0){paths.push(path);return;}if(value!==null&&typeof value==='object'){for(const [key,item] of Object.entries(value))walk(item,path?path+'.'+key:key);}else paths.push(path);}
    walk(window['mdpdf'].state.theme);
    return paths.filter(path=>!document.querySelector(`[data-theme-path="${path}"]`));
  });
  expect(missing).toEqual([]);
  await page.locator('[data-theme-path="page.size"]').selectOption('custom');
  await page.locator('[data-theme-path="page.size.custom.width"]').fill('180');
  await expect.poll(()=>page.evaluate(()=>window['mdpdf'].state.theme.page.size.custom.width)).toBe(180);
});

test('PDF downloads, exact preview, and strict diagram diagnostics work',async({page})=>{
  await ready(page);
  await replace(page,'---\ntitle: Export check\nauthor: Writer\n---\n# A real PDF\n\nHello from the browser.');
  await expect(page.frameLocator('#live-preview').locator('h1')).toHaveText('A real PDF');
  const downloaded=page.waitForEvent('download');await page.locator('#export-pdf').click();const download=await downloaded;
  expect(download.suggestedFilename()).toMatch(/\.pdf$/);
  const file=await download.path();expect(readFileSync(file!).subarray(0,5).toString()).toBe('%PDF-');
  await page.locator('#pdf-mode').click();await expect(page.locator('#pdf-preview')).toHaveAttribute('src',/^blob:/);await expect(page.locator('#refresh-pdf')).toBeVisible();
  await page.locator('#live-mode').click();
  await replace(page,'```mermaid\nflowchart TD\n A[Unclosed\n```');
  await expect(page.locator('#diagnostics')).toBeVisible();await expect(page.locator('.cm-lintRange-error')).toHaveCount(1);
  await page.locator('.advanced-section summary').click();await page.locator('#strict-diagrams').check();
  const response=page.waitForResponse(r=>r.url().endsWith('/api/export'));await page.locator('#export-pdf').click();expect((await response).status()).toBe(422);
});

test('images and fonts upload into the current session and appear in preview',async({page})=>{
  await ready(page);await replace(page,'# Uploads\n\n');
  await page.locator('#image-file').setInputFiles(path.resolve('e2e/fixtures/pixel.png'));
  const image=page.frameLocator('#live-preview').locator('img');await expect(image).toHaveCount(1);await expect(image).toHaveAttribute('src',/^data:image\/png;base64,/);
  await expect.poll(()=>image.evaluate(img=>(img as HTMLImageElement).naturalWidth)).toBe(1);
  await page.locator('#font-file').setInputFiles(path.resolve('assets/fonts/jetbrains-mono/jetbrains-mono-latin-wght-normal.woff2'));
  await expect.poll(()=>page.evaluate(()=>window['mdpdf'].state.theme.typography.body_font)).toMatch(/^assetfont-/);
  await expect.poll(()=>page.frameLocator('#live-preview').locator('#mdpdf-custom-fonts').count()).toBe(1);
  const family=await page.evaluate(()=>window['mdpdf'].state.theme.typography.body_font);
  await expect(page.frameLocator('#live-preview').locator('html')).toHaveCSS('font-family',new RegExp(family));
  await page.locator('#font-file').setInputFiles(path.resolve('assets/fonts/merriweather/merriweather-latin-wght-normal.woff2'));
  await expect.poll(()=>page.evaluate(()=>window['mdpdf'].state.fonts.length)).toBe(2);
  await expect.poll(()=>page.evaluate(()=>window['mdpdf'].state.theme.typography.body_font)).not.toBe(family);
  await expect(page.frameLocator('#live-preview').locator('#mdpdf-custom-fonts')).toHaveCount(1);
  await page.locator('summary').filter({hasText:/^Typography$/}).click();
  let requests=0;page.on('request',r=>{if(r.url().includes('/api/rpc/render_preview'))requests++;});
  await page.locator('[data-theme-path="typography.heading_font"]').selectOption(family);
  await expect(page.frameLocator('#live-preview').locator('h1')).toHaveCSS('font-family',new RegExp(family));
  await expect.poll(()=>page.frameLocator('#live-preview').locator('h1').evaluate((el,family)=>[...el.ownerDocument.fonts].some(face=>face.family===family&&face.status==='loaded'),family)).toBe(true);
  expect(requests).toBe(0);
});

test('responsive tabs, file open and UI color theme are usable',async({page})=>{
  await page.setViewportSize({width:390,height:844});await ready(page);
  await page.locator('#document-menu').click();await page.locator('#menu-ui-theme').click();await expect(page.locator('html')).toHaveClass(/light-ui/);
  await expect(page.locator('.editor-pane')).toBeVisible();await expect(page.locator('.preview-pane')).toBeHidden();
  await page.locator('[data-pane="preview"]').click();await expect(page.locator('.preview-pane')).toBeVisible();
  await page.locator('[data-pane="settings"]').click();await expect(page.locator('.settings-pane')).toBeVisible();
  await page.locator('#markdown-file').setInputFiles({name:'opened.md',mimeType:'text/markdown',buffer:Buffer.from('# Opened document\n\nFrom a file.')});
  await page.locator('[data-pane="preview"]').click();await expect(page.frameLocator('#live-preview').locator('h1')).toHaveText('Opened document');
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
});

test('math, cover pages and uploaded syntax themes share preview and PDF',async({page})=>{
  await ready(page);
  await replace(page,'---\ntitle: Equations\nauthor: Ada\ncover: true\n---\n# Mathematics\n\nEuler: $e^{i\\pi}+1=0$.\n\n$$\\sum_{n=1}^{10} n$$\n\n```rust\nlet answer = 42;\n```');
  const preview=page.frameLocator('#live-preview');
  await expect(preview.locator('.mdpdf-cover h1')).toHaveText('Equations');
  await expect(preview.locator('.katex')).toHaveCount(2);
  await expect(preview.locator('.katex').first()).toHaveCSS('font-family',/KaTeX_Main/);
  await page.locator('summary').filter({hasText:/^Code blocks$/}).click();
  await page.locator('[data-section="code"] input[type="file"]').setInputFiles(path.resolve('assets/syntax/nord.tmTheme'));
  await expect.poll(()=>page.evaluate(()=>window['mdpdf'].state.theme.code.custom_syntax_css)).toContain('.mdpdf-code');
  const response=page.waitForResponse(r=>r.url().endsWith('/api/export'));await page.locator('#export-pdf').click();expect((await response).status()).toBe(200);
});

test('scroll sync moves both panes without feedback loops',async({page})=>{
  await ready(page);
  await replace(page,Array.from({length:65},(_,i)=>`## Section ${i+1}\n\nParagraph ${i+1} with enough content to make a long document.\n`).join('\n'));
  await expect(page.frameLocator('#live-preview').locator('h2')).toHaveCount(65);
  await page.evaluate(()=>{window['mdpdf'].editor.scrollDOM.scrollTop=2000;});
  await expect.poll(()=>page.locator('#preview-scroll').evaluate(el=>el.scrollTop)).toBeGreaterThan(100);
  await page.waitForTimeout(500);
  await page.locator('#preview-scroll').evaluate(el=>el.scrollTop=0);
  await expect.poll(()=>page.evaluate(()=>window['mdpdf'].editor.scrollDOM.scrollTop)).toBeLessThan(100);
});

test('structural theme controls update locally using the shared WASM renderer',async({page})=>{
  await ready(page);
  await replace(page,'# Title\n\n## Details\n\n```rust\nlet first = 1;\nlet second = 2;\n```');
  const preview=page.frameLocator('#live-preview');await expect(preview.locator('h1')).toHaveText('Title');
  let requests=0;page.on('request',r=>{if(r.url().includes('/api/rpc/render_preview'))requests++;});
  await page.locator('[data-theme-path="page.toc"]').check();await expect(preview.locator('.mdpdf-toc')).toBeVisible();
  await page.locator('[data-theme-path="page.heading_numbers"]').check();await expect(preview.locator('h2')).toHaveText('1.1 Details');
  await page.locator('summary').filter({hasText:/^Code blocks$/}).click();
  await page.locator('[data-theme-path="code.line_numbers"]').check();await expect(preview.locator('.line-numbers')).toHaveText('1\n2');
  await page.locator('#preset').selectOption('compact');
  await expect(preview.locator('h1')).toHaveText('Title');
  await expect(preview.locator('.line-numbers')).toHaveCount(0);
  expect(requests).toBe(0);
});
