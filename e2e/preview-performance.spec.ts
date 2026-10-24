import { test, expect } from '@playwright/test';

async function ready(page) {
  await page.goto('/');
  await page.waitForFunction(() => window['mdpdf']?.ready);
}

async function open(page, text: string, name = 'performance.md') {
  const generation = await page.evaluate(() => window['mdpdf'].state.generation);
  await page.locator('#markdown-file').setInputFiles({name, mimeType:'text/markdown', buffer:Buffer.from(text)});
  await page.waitForFunction(generation => window['mdpdf'].ready && window['mdpdf'].state.generation > generation, generation);
}

test('file open and edits reuse the sandbox and unchanged diagrams without preview requests', async ({page}) => {
  await ready(page);
  const preview = page.frameLocator('#live-preview');
  await preview.locator('svg').first().evaluate(node => { window['retainedDiagram'] = node; window['retainedDocument'] = document; });
  let requests = 0;
  page.on('request', request => { if (request.url().includes('/api/rpc/render_preview')) requests++; });
  const source = await page.evaluate(() => window['mdpdf'].state.markdown);
  await open(page, source + '\n\nA new paragraph.');
  expect(requests).toBe(0);
  expect(await preview.locator('svg').first().evaluate(node => node === window['retainedDiagram'] && document === window['retainedDocument'])).toBe(true);
  await open(page, '# Short file\n\nA much shorter document.');
  await expect(preview.locator('h1')).toHaveText('Short file');
  expect(requests).toBe(0);
  await expect.poll(() => page.evaluate(() => window['mdpdf'].state.height)).toBeLessThan(1500);
});

test('dropping Markdown on CodeMirror opens exactly one document', async ({page}) => {
  await ready(page);
  const text = '# Dropped file\n\nOpened from a file drop.';
  await page.locator('#editor .cm-content').evaluate((node, text) => {
    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(new File([text], 'dropped.md', {type:'text/markdown'}));
    node.dispatchEvent(new DragEvent('drop', {bubbles:true, cancelable:true, dataTransfer}));
  }, text);
  await expect(page.frameLocator('#live-preview').locator('h1')).toHaveText('Dropped file');
  expect(await page.evaluate(() => window['mdpdf'].state.markdown)).toBe(text);
  await expect(page.locator('#filename')).toHaveValue('dropped.md');
});

test('long document scroll synchronization uses cached geometry', async ({page}) => {
  await ready(page);
  await open(page, Array.from({length:300}, (_,i) => `## Section ${i + 1}\n\nParagraph with **formatted text** and a [link](https://example.com).\n\n- One\n- Two\n`).join('\n'));
  const preview = page.frameLocator('#live-preview');
  await expect(preview.locator('h2')).toHaveCount(300);
  const reads = await preview.locator('main').evaluate(async () => {
    // Let the resize observer and the post-render index finish first.
    await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
    let reads = 0;
    const original = Element.prototype.getBoundingClientRect;
    Element.prototype.getBoundingClientRect = function() { reads++; return original.call(this); };
    try {
      for (let i = 0; i < 120; i++) window.dispatchEvent(new MessageEvent('message', {source:parent, data:{mdpdf:true, type:'position-line', y:i*200}}));
      await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
      return reads;
    } finally { Element.prototype.getBoundingClientRect = original; }
  });
  expect(reads).toBeLessThan(20);
  await page.waitForTimeout(300);
  await page.evaluate(() => { window['mdpdf'].editor.scrollDOM.scrollTop = 5000; });
  await expect.poll(() => page.locator('#preview-scroll').evaluate(node => node.scrollTop)).toBeGreaterThan(500);
});

test('the latest file wins when an older native asset request is slow', async ({page}) => {
  await ready(page);
  let release;
  const held = new Promise<void>(resolve => { release = resolve; });
  let intercepted;
  const requested = new Promise<void>(resolve => { intercepted = resolve; });
  await page.route('**/api/rpc/render_preview_update', async route => {
    intercepted(); await held;
    await route.continue().catch(() => {});
  });
  await page.locator('#markdown-file').setInputFiles({name:'older.md', mimeType:'text/markdown', buffer:Buffer.from('# Older document\n\n日本語')});
  await requested;
  await open(page, '# Latest document\n\nThis file should remain visible.', 'latest.md');
  release();
  await expect(page.frameLocator('#live-preview').locator('h1')).toHaveText('Latest document');
  await expect(page.locator('#filename')).toHaveValue('latest.md');
});

test('theme changes supersede a preview request already in flight', async ({page}) => {
  await ready(page);
  let release;
  const held = new Promise<void>(resolve => { release = resolve; });
  let intercepted;
  const requested = new Promise<void>(resolve => { intercepted = resolve; });
  let requests = 0;
  await page.route('**/api/rpc/render_preview_update', async route => {
    if (++requests === 1) { intercepted(); await held; }
    await route.continue().catch(() => {});
  });
  await page.locator('#markdown-file').setInputFiles({name:'theme-race.md', mimeType:'text/markdown', buffer:Buffer.from('# Current document\n\n日本語')});
  await requested;
  await page.locator('summary').filter({hasText:/^Typography$/}).click();
  await page.locator('[data-theme-path="typography.text_color"]').evaluate((input:HTMLInputElement) => { input.value='#314a27'; input.dispatchEvent(new Event('input',{bubbles:true})); });
  await page.waitForFunction(() => window['mdpdf'].ready);
  release();
  await expect(page.frameLocator('#live-preview').locator('h1')).toHaveText('Current document');
  await expect(page.frameLocator('#live-preview').locator('html')).toHaveCSS('color','rgb(49, 74, 39)');
});

test('native updates still work when a preview worker cannot start', async ({page}) => {
  await page.addInitScript(() => { window.Worker = class { constructor() { throw new Error('Worker unavailable'); } } as any; });
  await ready(page);
  await page.frameLocator('#live-preview').locator('main').evaluate(() => { window['originalDocument'] = document; });
  await open(page, '# Native fallback\n\nThis preview still updates.');
  const heading = page.frameLocator('#live-preview').locator('h1');
  await expect(heading).toHaveText('Native fallback');
  expect(await heading.evaluate(() => window['originalDocument'] === document)).toBe(true);
});

test('new text appears while native font delivery is pending', async ({page}) => {
  await ready(page);
  let release;
  const held = new Promise<void>(resolve => { release = resolve; });
  await page.route('**/api/rpc/render_preview_update', async route => { await held; await route.continue().catch(() => {}); });
  await page.locator('#markdown-file').setInputFiles({name:'font-delivery.md', mimeType:'text/markdown', buffer:Buffer.from('# Immediate text\n\n日本語\n\n```mermaid\nflowchart LR\n A-->B\n```')});
  await expect(page.frameLocator('#live-preview').locator('h1')).toHaveText('Immediate text');
  expect(await page.evaluate(() => window['mdpdf'].ready)).toBe(false);
  expect(await page.frameLocator('#live-preview').locator('pre[data-diagram-id] svg').count()).toBe(0);
  release();
  await page.waitForFunction(() => window['mdpdf'].ready);
  await expect(page.frameLocator('#live-preview').locator('pre[data-diagram-id] svg')).toHaveCount(1);
});

test('opening another file cancels the remaining diagrams from the old document', async ({page}) => {
  await ready(page);
  const preview = page.frameLocator('#live-preview');
  await preview.locator('main').evaluate(() => {
    const original = window['mermaid'].render;
    window['diagramCalls'] = 0;
    window['mermaid'].render = async function(...args) {
      window['diagramCalls']++;
      if (window['diagramCalls'] === 1) await new Promise(resolve => { window['releaseDiagram'] = resolve; });
      return original.apply(this,args);
    };
  });
  const old = '# Older diagrams\n\n' + '```mermaid\nflowchart LR\n Old-->Diagram\n```\n\n'.repeat(12);
  await page.locator('#markdown-file').setInputFiles({name:'older-diagrams.md',mimeType:'text/markdown',buffer:Buffer.from(old)});
  await expect.poll(() => preview.locator('main').evaluate(() => window['diagramCalls'])).toBe(1);
  await page.locator('#markdown-file').setInputFiles({name:'newest.md',mimeType:'text/markdown',buffer:Buffer.from('# Latest file\n\nThe old diagrams are no longer needed.')});
  await expect(preview.locator('h1')).toHaveText('Latest file');
  await preview.locator('main').evaluate(() => window['releaseDiagram']());
  await page.waitForFunction(() => window['mdpdf'].ready);
  expect(await preview.locator('main').evaluate(() => window['diagramCalls'])).toBe(1);
  await expect(preview.locator('pre[data-diagram-id]')).toHaveCount(0);
});
