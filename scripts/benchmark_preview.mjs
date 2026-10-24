// Exercise real file drops, diagram completion and scrolling with local samples.
import {chromium} from 'playwright';
import {readFileSync, readdirSync, mkdirSync, writeFileSync} from 'node:fs';
import {resolve, basename} from 'node:path';
import assert from 'node:assert/strict';

const source = resolve(process.argv[2] || 'markdown');
const root = process.env.MDPDF_BASE_URL || 'http://127.0.0.1:3000';
const browser = await chromium.launch({executablePath:process.env.CHROME_PATH || chromium.executablePath(), chromiumSandbox:true});
const results = [];
try {
  const page = await browser.newPage({viewport:{width:1512,height:982}});
  const errors = [];
  page.on('pageerror',error => errors.push(error.message));
  let requests = 0, frameLoads = 0;
  page.on('request',request => { if (request.url().includes('/api/rpc/render_preview')) requests++; });
  page.on('framenavigated',frame => { if (frame.parentFrame()) frameLoads++; });
  await page.goto(root);
  await page.waitForFunction(() => window.mdpdf?.ready);
  await page.evaluate(() => window.addEventListener('message',event => {
    const sample = window.previewBenchmark;
    if (sample && sample.contentAt === null && event.source === document.getElementById('live-preview').contentWindow && event.data?.type === 'content' && event.data.revision > sample.generation && event.data.revision === window.mdpdf.state.revision) sample.contentAt = performance.now();
  }));
  for (const file of readdirSync(source).filter(file => file.endsWith('.md')).sort()) {
    const text = readFileSync(resolve(source,file),'utf8');
    const before = {requests, frameLoads};
    const generation = await page.evaluate(() => window.mdpdf.state.generation);
    const started = performance.now();
    await page.locator('#editor .cm-content').evaluate((node,{text,file}) => {
      window.previewBenchmark = {started:performance.now(),generation:window.mdpdf.state.generation,contentAt:null};
      const dataTransfer = new DataTransfer();
      dataTransfer.items.add(new File([text],file,{type:'text/markdown'}));
      node.dispatchEvent(new DragEvent('drop',{bubbles:true,cancelable:true,dataTransfer}));
    },{text,file});
    await page.waitForFunction(generation => window.mdpdf.ready && window.mdpdf.state.generation > generation,generation,{timeout:120000});
    const dropToReadyMs = Math.round(performance.now() - started);
    assert.equal(await page.evaluate(() => window.mdpdf.state.markdown),text,'File drop altered the source');
    const timing = await page.evaluate(() => ({dropToContentMs:Math.round(window.previewBenchmark.contentAt-window.previewBenchmark.started),contentUpdateMs:Math.round(window.mdpdf.lastContentMs),renderMs:Math.round(window.mdpdf.lastRenderMs)}));
    const frame = page.frames().find(frame => frame.parentFrame());
    const details = await frame.evaluate(async () => {
      await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
      let geometryReads = 0;
      const original = Element.prototype.getBoundingClientRect;
      Element.prototype.getBoundingClientRect = function() { geometryReads++; return original.call(this); };
      try {
        for (let i=0;i<120;i++) window.dispatchEvent(new MessageEvent('message',{source:parent,data:{mdpdf:true,type:'position-line',y:i*100}}));
        await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
        return {diagrams:document.querySelectorAll('pre[data-diagram-id]').length,renderedDiagrams:document.querySelectorAll('pre[data-diagram-id] svg').length,diagnostics:window.__MDPDF_ERRORS__ || [],scrollGeometryReads:geometryReads};
      } finally { Element.prototype.getBoundingClientRect = original; }
    });
    assert.equal(details.diagrams,details.renderedDiagrams,file+': unfinished diagrams');
    assert.deepEqual(details.diagnostics,[],file+': diagram errors');
    assert.equal(frameLoads,before.frameLoads,file+': preview iframe reloaded');
    results.push({file,bytes:Buffer.byteLength(text),dropToReadyMs,...timing,previewRequests:requests-before.requests,...details});
    console.log(JSON.stringify(results.at(-1)));
  }
  assert.deepEqual(errors,[],'Browser errors');
} finally { await browser.close(); }
mkdirSync('target/verification',{recursive:true});
const report = {source:basename(source),testedAt:new Date().toISOString(),results};
writeFileSync('target/verification/preview-performance.json',JSON.stringify(report,null,2)+'\n');
