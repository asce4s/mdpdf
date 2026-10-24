// Real Chromium check of core-built, embedded fixtures in opaque sandboxed iframes.
// Uses Node built-ins, Cargo and local Chromium; no npm dependencies.
const { spawn, spawnSync } = require('node:child_process');
const { createServer } = require('node:http');
const { readFileSync, existsSync, mkdtempSync, rmSync } = require('node:fs');
const { resolve, extname } = require('node:path');
const { tmpdir } = require('node:os');
const assert = require('node:assert/strict');
const root = resolve(__dirname, '..');
const profile = mkdtempSync(resolve(tmpdir(), 'mdpdf-chrome-'));
const artifacts = mkdtempSync(resolve(tmpdir(), 'mdpdf-html-'));
const contexts = new Map();
const loads = new Map();
const server = createServer((req, res) => {
  if (req.url === '/') { res.setHeader('Content-Type', 'text/html'); res.end('<!doctype html><title>Browser check</title>'); return; }
  const path = resolve(root, '.' + decodeURIComponent(req.url));
  if (!path.startsWith(root + '/') || !existsSync(path)) { res.writeHead(404); res.end(); return; }
  const types = { '.js': 'text/javascript', '.woff2': 'font/woff2', '.css': 'text/css' };
  res.setHeader('Content-Type', types[extname(path)] || 'text/plain');
  res.end(readFileSync(path));
});
let chrome;
const pending = new Map();
let sequence = 0, buffer = '';
function command(method, params = {}, sessionId) {
  const id = ++sequence;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => { pending.delete(id); reject(new Error('CDP timeout: ' + method)); }, 40000);
    pending.set(id, message => { clearTimeout(timer); message.error ? reject(new Error(JSON.stringify(message.error))) : resolve(message.result); });
    chrome.stdio[3].write(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }) + '\0');
  });
}
async function main() {
  const assets = { '/assets/vendor/mermaid/mermaid.min.js': 'data:text/javascript;base64,' + readFileSync(resolve(root, 'assets/vendor/mermaid/mermaid.min.js')).toString('base64') };
  assets['/assets/vendor/katex/katex.min.js'] = 'data:text/javascript;base64,' + readFileSync(resolve(root, 'assets/vendor/katex/katex.min.js')).toString('base64');
  const css = readFileSync(resolve(root, 'assets/fonts/fonts.css'), 'utf8') + readFileSync(resolve(root, 'assets/vendor/katex/katex.min.css'), 'utf8');
  for (const match of css.matchAll(/url\((\/assets\/(?:fonts|vendor\/katex\/fonts)\/[^)]+)\)/g)) {
    assets[match[1]] = 'data:font/woff2;base64,' + readFileSync(resolve(root, '.' + match[1])).toString('base64');
  }
  require('node:fs').writeFileSync(resolve(artifacts, 'assets.json'), JSON.stringify(assets));
  const built = spawnSync('cargo', ['run', '--offline', '--quiet', '-p', 'mdpdf-core', '--example', 'fixture_html', '--', resolve(artifacts, 'assets.json'), artifacts], { cwd: root, encoding: 'utf8' });
  assert.equal(built.status, 0, built.stderr);

  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const origin = `http://127.0.0.1:${server.address().port}`;
  chrome = spawn(process.env.CHROME_PATH || 'chromium', ['--headless=new', '--disable-gpu', '--remote-debugging-pipe', '--no-first-run', `--user-data-dir=${profile}`], { stdio: ['ignore', 'ignore', 'pipe', 'pipe', 'pipe'] });
  chrome.stderr.on('data', data => { if (process.env.DEBUG_CHROME) process.stderr.write(data); });
  chrome.stdio[4].on('data', data => {
    buffer += data;
    let end;
    while ((end = buffer.indexOf('\0')) >= 0) {
      const message = JSON.parse(buffer.slice(0, end)); buffer = buffer.slice(end + 1);
      if (message.method === 'Page.loadEventFired' && loads.has(message.sessionId)) { loads.get(message.sessionId)(); loads.delete(message.sessionId); }
      if (message.method === 'Target.detachedFromTarget') for (const [frame, context] of contexts) if (context.sessionId === message.params.sessionId) contexts.delete(frame);
      if (message.method === 'Target.attachedToTarget') command('Runtime.enable', {}, message.params.sessionId).catch(console.error);
      if (message.method === 'Runtime.executionContextCreated' && message.params.context.auxData?.isDefault) contexts.set(message.params.context.auxData.frameId, { id: message.params.context.id, sessionId: message.sessionId });
      if (message.method === 'Runtime.executionContextDestroyed') for (const [frame, id] of contexts) if (id.id === message.params.executionContextId && id.sessionId === message.sessionId) contexts.delete(frame);
      if (process.env.DEBUG_CHROME && ['Log.entryAdded','Network.loadingFailed'].includes(message.method)) console.error(JSON.stringify(message.params));
      if (pending.has(message.id)) { pending.get(message.id)(message); pending.delete(message.id); }
    }
  });
  const { targetId } = await command('Target.createTarget', { url: origin });
  const { sessionId } = await command('Target.attachToTarget', { targetId, flatten: true });
  const send = (method, params) => command(method, params, sessionId);
  await send('Page.enable');
  await send('Log.enable');
  await send('Network.enable');
  await send('Runtime.enable');
  await send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });
  for (const name of ['mermaid-all', 'mermaid-error', 'mermaid-init-directive', 'fonts-cjk', 'xss', 'math-cover']) {
    const html = readFileSync(resolve(artifacts, name + '.html'), 'utf8');
    const loaded = new Promise(resolve => loads.set(sessionId, resolve));
    await send('Page.navigate', { url: origin });
    await loaded;
    const { frameTree } = await send('Page.getFrameTree');
    const escaped = html.replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
    await send('Page.setDocumentContent', { frameId: frameTree.frame.id, html: `<iframe sandbox="allow-scripts" srcdoc="${escaped}"></iframe>` });
    let child;
    for (let retry = 0; retry < 200; retry++) {
      const tree = await send('Page.getFrameTree');
      child = tree.frameTree.childFrames?.[0]?.frame;
      if (!child) { const id = [...contexts.keys()].find(id => id !== tree.frameTree.frame.id); if (id) child = { id }; }
      if (child && contexts.has(child.id)) break;
      await new Promise(resolve => setTimeout(resolve, 25));
    }
    assert.ok(child && contexts.has(child.id), 'sandbox frame has no execution context');
    const frameContext = contexts.get(child.id);
    const { result, exceptionDetails } = await command('Runtime.evaluate', { expression: `(async () => {
      const deadline = Date.now() + 35000;
      while (!window.__MDPDF_READY__ && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 25));
      return { origin: window.origin, ready: window.__MDPDF_READY__, errors: window.__MDPDF_ERRORS__,
        svg: [...document.querySelectorAll('figure.mdpdf-diagram')].filter(figure => figure.querySelector('svg')).length,
        diagrams: document.querySelectorAll('figure.mdpdf-diagram').length,
        math: document.querySelectorAll('.katex').length,
        svgIds: [...document.querySelectorAll('pre[data-diagram-id]')].map(pre => pre.querySelector('svg')?.id),
        fonts: [...document.fonts].filter(f => f.status === 'loaded').map(f => f.family),
        registered: window.mermaid?.getRegisteredDiagramsMetadata?.(),
        clippedDiagrams: [...document.querySelectorAll('figure.mdpdf-diagram > pre > svg')].flatMap(svg => {
          const box = svg.getBBox(), view = svg.viewBox.baseVal;
          return view.width && (box.x < view.x - 3 || box.y < view.y - 3 || box.x + box.width > view.x + view.width + 3 || box.y + box.height > view.y + view.height + 3) ? [{ diagram: svg.closest('figure').querySelector('figcaption')?.textContent, bounds: [box.x,box.y,box.width,box.height], view: [view.x,view.y,view.width,view.height] }] : [];
        }),
        rasterEffects: [...document.querySelectorAll('figure.mdpdf-diagram svg *')].filter(el => getComputedStyle(el).filter !== 'none').map(el => ({tag:el.tagName,filter:getComputedStyle(el).filter, diagram:el.closest('figure').querySelector('figcaption')?.textContent})),
        body: document.querySelector('main').innerHTML };
    })()`, contextId: frameContext.id, awaitPromise: true, returnByValue: true }, frameContext.sessionId);
    assert.ok(!exceptionDetails, JSON.stringify(exceptionDetails));
    const value = result.value;
    if (value.svgIds) assert.equal(new Set(value.svgIds).size,value.svgIds.length, name + ": duplicate diagram IDs");
    console.log(name, JSON.stringify({ ...value, body: undefined, registered: undefined, fonts: [...new Set(value.fonts)] }));
    assert.equal(value.origin, 'null', 'iframe must have an opaque origin');
    assert.deepEqual(value.rasterEffects, [], 'SVG effects would rasterize during PDF export');
    assert.deepEqual(value.clippedDiagrams, [], 'SVG extends beyond its viewBox');
    assert.equal(value.ready, true, name + ' never became ready');
    if (name === 'mermaid-error') { if (value.errors.length !== 1) console.error(value.body.slice(0, 1600)); assert.equal(value.errors.length, 1); }
    else { assert.deepEqual(value.errors, [], name); assert.equal(value.svg, value.diagrams); }
    if (name === 'math-cover') assert.equal(value.math, 2);
    if (name === 'xss') assert.ok(!/<script|onerror|javascript:/.test(value.body));
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; }).finally(async () => {
  server.close();
  if (chrome) { const done = new Promise(resolve => chrome.once('exit', resolve)); chrome.kill(); await done; }
  rmSync(profile, { recursive: true, force: true });
  rmSync(artifacts, { recursive: true, force: true });
});
