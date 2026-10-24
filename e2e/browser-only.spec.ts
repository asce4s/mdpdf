import {test,expect} from '@playwright/test';
import {readFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {execFileSync} from 'node:child_process';

async function ready(page){await page.goto('./');await page.waitForFunction(()=>window['mdpdf']?.ready);}
async function open(page,text:string,name='document.md'){
  const generation=await page.evaluate(()=>window['mdpdf'].state.generation);
  await page.locator('#markdown-file').setInputFiles({name,mimeType:'text/markdown',buffer:Buffer.from(text)});
  await page.waitForFunction(generation=>window['mdpdf'].ready&&window['mdpdf'].state.generation>generation,generation);
}
async function drop(page,files:{name:string,type:string,text?:string,bytes?:number[]}[]){
  await page.locator('#editor .cm-content').evaluate((node,files)=>{
    const dataTransfer=new DataTransfer();
    for(const file of files)dataTransfer.items.add(new File([file.bytes?new Uint8Array(file.bytes):file.text!],file.name,{type:file.type}));
    node.dispatchEvent(new DragEvent('drop',{bubbles:true,cancelable:true,dataTransfer}));
  },files);
}

test('starts beneath a Pages project path using only static GET requests',async({page})=>{
  const errors:string[]=[],failures:string[]=[],requests:any[]=[];
  page.on('pageerror',error=>errors.push(error.message));
  page.on('response',response=>{if(response.status()>=400)failures.push(response.url());});
  page.on('request',request=>requests.push({url:request.url(),method:request.method()}));
  await ready(page);
  await expect(page.frameLocator('#live-preview').locator('h1')).toContainText('A better way');
  await expect(page.frameLocator('#live-preview').locator('pre[data-diagram-id] svg')).toHaveCount(1);
  await expect(page.locator('#live-preview')).toHaveAttribute('sandbox','allow-scripts allow-modals');
  expect(await page.frameLocator('#live-preview').locator('html').evaluate(()=>window.origin)).toBe('null');
  expect(requests.filter(request=>/^https?:/.test(request.url)).every(request=>request.method==='GET'&&new URL(request.url).pathname.startsWith('/mdpdf/'))).toBe(true);
  expect(requests.some(request=>new URL(request.url).pathname.includes('/api/'))).toBe(false);
  expect(errors).toEqual([]);expect(failures).toEqual([]);
  await expect(page.getByRole('button',{name:'Print / Save PDF'})).toBeEnabled();
  await expect(page.locator('#pdf-mode')).toHaveCount(0);
});

test('the same static files work at the origin root',async({page})=>{
  await page.goto(process.env.MDPDF_BROWSER_ROOT_URL||'http://127.0.0.1:4174/');
  await page.waitForFunction(()=>window['mdpdf']?.ready);
  await expect(page.frameLocator('#live-preview').locator('h1')).toContainText('A better way');
});

test('file drops and editing retain the sandbox and existing SVGs',async({page})=>{
  await ready(page);
  const preview=page.frameLocator('#live-preview');
  await preview.locator('svg').first().evaluate(svg=>{window['originalSvg']=svg;window['originalDocument']=document;});
  const source=await page.evaluate(()=>window['mdpdf'].state.markdown);
  await open(page,source+'\n\nAn appended paragraph.');
  expect(await preview.locator('svg').first().evaluate(svg=>svg===window['originalSvg']&&document===window['originalDocument'])).toBe(true);
  const text='# Dropped exactly once\n\nLocal document.';
  await drop(page,[{name:'dropped.md',type:'text/markdown',text}]);
  await expect(preview.locator('h1')).toHaveText('Dropped exactly once');
  expect(await page.evaluate(()=>window['mdpdf'].state.markdown)).toBe(text);
  await expect(page.locator('#filename')).toHaveValue('dropped.md');
});

test('themes, structure and TextMate imports run locally',async({page})=>{
  await ready(page);
  const api:string[]=[];page.on('request',r=>{if(r.url().includes('/api/'))api.push(r.url());});
  await open(page,'# Title\n\n## Details\n\n```rust\nlet first = 1;\nlet second = 2;\n```');
  const preview=page.frameLocator('#live-preview');
  await page.locator('[data-theme-path="page.toc"]').check();await expect(preview.locator('.mdpdf-toc')).toBeVisible();
  await page.locator('[data-theme-path="page.heading_numbers"]').check();await expect(preview.locator('h2')).toHaveText('1.1 Details');
  await page.locator('summary').filter({hasText:/^Code blocks$/}).click();
  await page.locator('[data-theme-path="code.line_numbers"]').check();await expect(preview.locator('.line-numbers')).toHaveText('1\n2');
  await page.locator('[data-section="code"] input[type="file"]').setInputFiles(resolve('assets/syntax/nord.tmTheme'));
  await expect.poll(()=>page.evaluate(()=>window['mdpdf'].state.theme.code.custom_syntax_css)).toContain('.mdpdf-code');
  await page.locator('#preset').selectOption('compact');await expect(preview.locator('.line-numbers')).toHaveCount(0);
  expect(api).toEqual([]);
});

test('relative and reference images dropped with Markdown persist in IndexedDB',async({page})=>{
  await ready(page);
  const text='# Local images\n\n![One](./images/pixel.png){width=50%}\n\n![Two][picture]\n\n[picture]: pixel.png';
  await drop(page,[{name:'pictures.md',type:'text/markdown',text},{name:'pixel.png',type:'image/png',bytes:[...readFileSync('e2e/fixtures/pixel.png')]}]);
  const images=page.frameLocator('#live-preview').locator('img');
  await expect(images).toHaveCount(2);
  await expect(images.first()).toHaveAttribute('src',/^data:image\/png;base64,/);
  await expect.poll(()=>images.first().evaluate((img:HTMLImageElement)=>img.naturalWidth)).toBe(1);
  expect(await page.evaluate(()=>window['mdpdf'].state.markdown)).toBe(text);
  await page.waitForTimeout(2200);await page.reload();await page.waitForFunction(()=>window['mdpdf']?.ready);
  await expect(images).toHaveCount(2);await expect.poll(()=>images.last().evaluate((img:HTMLImageElement)=>img.naturalWidth)).toBe(1);
  await expect(page.locator('#diagnostics')).toBeHidden();
});

test('imported fonts and TOML themes persist without uploads',async({page})=>{
  await ready(page);await open(page,'# Imported typography\n\nA local font.');
  await page.locator('#font-file').setInputFiles(resolve('assets/fonts/merriweather/merriweather-latin-wght-normal.woff2'));
  await expect.poll(()=>page.evaluate(()=>window['mdpdf'].state.theme.typography.body_font)).toMatch(/^assetfont-/);
  const family=await page.evaluate(()=>window['mdpdf'].state.theme.typography.body_font);
  await expect(page.frameLocator('#live-preview').locator('html')).toHaveCSS('font-family',new RegExp(family));
  await page.waitForTimeout(2200);await page.reload();await page.waitForFunction(()=>window['mdpdf']?.ready);
  await expect(page.frameLocator('#live-preview').locator('html')).toHaveCSS('font-family',new RegExp(family));
  await page.locator('#theme-file').setInputFiles({name:'local.mdpdf-theme.toml',mimeType:'text/plain',buffer:Buffer.from('name = "Browser theme"\n[typography]\nbase_size = 12\n')});
  await expect.poll(()=>page.evaluate(()=>window['mdpdf'].state.theme.name)).toBe('Browser theme');
  await expect(page.frameLocator('#live-preview').locator('html')).toHaveCSS('font-size','16px');
  const downloaded=page.waitForEvent('download');await page.locator('#export-theme').click();
  const file=await (await downloaded).path();expect(readFileSync(file!,'utf8')).toContain('Browser theme');
});

test('CORS-enabled remote images resolve in the browser',async({page})=>{
  await page.route('https://images.example.test/pixel.png',route=>route.fulfill({status:200,contentType:'image/png',headers:{'access-control-allow-origin':'*'},body:readFileSync('e2e/fixtures/pixel.png')}));
  await ready(page);await open(page,'# Remote image\n\n![Pixel](https://images.example.test/pixel.png)');
  const image=page.frameLocator('#live-preview').locator('img');
  await expect(image).toHaveAttribute('src',/^data:image\/png;base64,/);
  await expect.poll(()=>image.evaluate((img:HTMLImageElement)=>img.naturalWidth)).toBe(1);
  await expect(page.locator('#diagnostics')).toBeHidden();
});

test('strict diagram errors block printing and valid documents open the browser print flow',async({page})=>{
  await ready(page);
  const preview=page.frameLocator('#live-preview');
  await preview.locator('html').evaluate(()=>window.addEventListener('beforeprint',()=>window['beforePrintCount']=(window['beforePrintCount']||0)+1));
  await open(page,'```mermaid\nflowchart TD\n A[Unclosed\n```');
  await expect(page.locator('#diagnostics')).toBeVisible();
  await page.locator('.advanced-section summary').click();await page.locator('#strict-diagrams').check();
  await page.locator('#export-pdf').click();await expect(page.locator('#toast')).toContainText('Fix diagram errors');
  expect(await preview.locator('html').evaluate(()=>window['beforePrintCount']||0)).toBe(0);
  await open(page,'# Printable document\n\nThis content is ready.');
  await page.locator('#export-pdf').click();
  await expect.poll(()=>preview.locator('html').evaluate(()=>window['beforePrintCount']||0)).toBe(1);
});

test('self-contained HTML renders math, cover pages and diagrams offline and prints to PDF',async({page,context})=>{
  await ready(page);
  await open(page,'---\ntitle: Browser mathematics\nauthor: Ada\ncover: true\n---\n# Mathematics\n\nEuler: $e^{i\\pi}+1=0$.\n\n```mermaid\nflowchart LR\n A[Write]-->B[Print]\n```');
  await expect(page.frameLocator('#live-preview').locator('.katex')).toHaveCount(1);
  await page.locator('#document-menu').click();
  const downloaded=page.waitForEvent('download');await page.locator('#download-html').click();
  const html=readFileSync((await (await downloaded).path())!,'utf8');
  const standalone=await context.newPage();const unexpected:string[]=[];
  await standalone.route('**/*',route=>{if(route.request().url()==='https://offline.example.test/document.html')return route.fulfill({contentType:'text/html',body:html});unexpected.push(route.request().url());return route.abort();});
  await standalone.goto('https://offline.example.test/document.html');
  await standalone.waitForFunction(()=>window['__MDPDF_READY__']);
  await expect(standalone.locator('.mdpdf-cover h1')).toHaveText('Browser mathematics');
  await expect(standalone.locator('.katex')).toHaveCount(1);await expect(standalone.locator('pre[data-diagram-id] svg')).toHaveCount(1);
  expect(unexpected).toEqual([]);
  mkdirSync('target/browser-verification',{recursive:true});
  const path=resolve('target/browser-verification/math-cover.pdf');
  await standalone.pdf({path,preferCSSPageSize:true,printBackground:true});
  expect(execFileSync('pdfinfo',[path],{encoding:'utf8'})).toMatch(/Pages:\s+2/);
  const text=execFileSync('pdftotext',[path,'-'],{encoding:'utf8'});
  expect(text).toContain('Browser mathematics');expect(text).toContain('Mathematics');
  await standalone.close();
});

test('large document scrolling uses cached geometry',async({page})=>{
  await ready(page);
  await open(page,Array.from({length:300},(_,i)=>`## Section ${i+1}\n\nParagraph with **bold text**.\n\n- One\n- Two\n`).join('\n'));
  const preview=page.frameLocator('#live-preview');await expect(preview.locator('h2')).toHaveCount(300);
  const reads=await preview.locator('main').evaluate(async()=>{
    await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);
    let count=0;const original=Element.prototype.getBoundingClientRect;
    Element.prototype.getBoundingClientRect=function(){count++;return original.call(this);};
    try{for(let i=0;i<120;i++)window.dispatchEvent(new MessageEvent('message',{source:parent,data:{mdpdf:true,type:'position-line',y:i*200}}));await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);return count;}
    finally{Element.prototype.getBoundingClientRect=original;}
  });
  expect(reads).toBeLessThan(20);
});

test('worker-unavailable browsers still render with local WASM',async({page})=>{
  await page.addInitScript(()=>{window.Worker=class{constructor(){throw new Error('Worker unavailable');}} as any;});
  await ready(page);await open(page,'# Local fallback\n\n$1+1=2$');
  await expect(page.frameLocator('#live-preview').locator('h1')).toHaveText('Local fallback');
  await expect(page.frameLocator('#live-preview').locator('.katex')).toHaveCount(1);
});

test('mobile panes and appearance remain usable',async({page})=>{
  await page.setViewportSize({width:390,height:844});await ready(page);
  await expect(page.locator('.editor-pane')).toBeVisible();
  await page.locator('[data-pane="preview"]').click();await expect(page.locator('.preview-pane')).toBeVisible();
  await page.locator('[data-pane="settings"]').click();await expect(page.locator('.settings-pane')).toBeVisible();
  await page.locator('#document-menu').click();await page.locator('#menu-ui-theme').click();await expect(page.locator('html')).toHaveClass(/light-ui/);
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
});

test('overlapping HTML exports both finish while preview work is queued',async({page})=>{
  await ready(page);await open(page,'# Parallel exports\n\n```mermaid\nflowchart LR\n A-->B\n```');
  const downloads:any[]=[];page.on('download',download=>downloads.push(download));
  await page.evaluate(()=>{const button=document.getElementById('download-html')!;button.click();button.click();});
  await expect.poll(()=>downloads.length).toBe(2);
  for(const download of downloads)expect(readFileSync((await download.path())!,'utf8')).toContain('Parallel exports');
});

test('opening a new file skips obsolete diagrams',async({page})=>{
  await ready(page);
  const preview=page.frameLocator('#live-preview');
  await preview.locator('main').evaluate(()=>{
    const original=window['mermaid'].render;window['diagramCalls']=0;
    window['mermaid'].render=async function(...args){window['diagramCalls']++;if(window['diagramCalls']===1)await new Promise(resolve=>{window['releaseDiagram']=resolve;});return original.apply(this,args);};
  });
  const text='# Older diagrams\n\n'+'```mermaid\nflowchart LR\n Old-->Diagram\n```\n\n'.repeat(12);
  await page.locator('#markdown-file').setInputFiles({name:'old.md',mimeType:'text/markdown',buffer:Buffer.from(text)});
  await expect.poll(()=>preview.locator('main').evaluate(()=>window['diagramCalls'])).toBe(1);
  await page.locator('#markdown-file').setInputFiles({name:'latest.md',mimeType:'text/markdown',buffer:Buffer.from('# Latest file\n\nThe previous diagrams are no longer needed.')});
  await expect(preview.locator('h1')).toHaveText('Latest file');
  await preview.locator('main').evaluate(()=>window['releaseDiagram']());
  await page.waitForFunction(()=>window['mdpdf'].ready);
  expect(await preview.locator('main').evaluate(()=>window['diagramCalls'])).toBe(1);
});

test('new CJK font subsets are supplied entirely from static files',async({page})=>{
  await ready(page);await open(page,readFileSync('crates/mdpdf-core/tests/fixtures/fonts-cjk.md','utf8'));
  const preview=page.frameLocator('#live-preview');
  await expect(preview.locator('pre[data-diagram-id] svg')).toHaveCount(1);
  await expect(page.locator('#diagnostics')).toBeHidden();
  expect(await preview.locator('html').evaluate(()=>[...document.fonts].some(face=>face.family==='Noto Sans CJK SC'&&face.status==='loaded'))).toBe(true);
});

test('image files larger than the Markdown limit still render locally',async({page})=>{
  await ready(page);await open(page,'# Large local image\n\n');
  const image=Buffer.concat([readFileSync('e2e/fixtures/pixel.png'),Buffer.alloc(2*1024*1024)]);
  await page.locator('#image-file').setInputFiles({name:'large.png',mimeType:'image/png',buffer:image});
  const preview=page.frameLocator('#live-preview').locator('img');
  await expect(preview).toHaveCount(1);
  await expect.poll(()=>preview.evaluate((img:HTMLImageElement)=>img.naturalWidth)).toBe(1);
  await expect(page.locator('#diagnostics')).toBeHidden();
});

test('document links open separately and preserve the trusted preview frame',async({page,context})=>{
  await context.route('https://external.example.test/**',route=>route.fulfill({contentType:'text/html',body:'<h1>External reference</h1>'}));
  await ready(page);await open(page,'# Local document\n\n[Reference](https://external.example.test/reference)');
  const preview=page.frameLocator('#live-preview');
  const opened=page.waitForEvent('popup');await preview.getByRole('link',{name:'Reference'}).click();
  const popup=await opened;await expect(popup.locator('h1')).toHaveText('External reference');
  expect(await popup.evaluate(()=>window.opener)).toBe(null);
  await expect(preview.locator('h1')).toHaveText('Local document');
  expect(await preview.locator('html').evaluate(()=>window.origin)).toBe('null');
  await popup.close();
});
