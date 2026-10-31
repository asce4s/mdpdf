(() => {
  'use strict';
  if (window.parent === window) return;
  const send = message => window.parent.postMessage({ mdpdf: true, ...message }, '*');
  const style = document.getElementById('mdpdf-theme');
  const fontStyle = document.createElement('style');
  fontStyle.id = 'mdpdf-fonts';
  const fontFaces = new Set(style.textContent.match(/@font-face\s*\{[^}]*\}/g) || []);
  fontStyle.textContent = [...fontFaces].join('\n');
  document.head.append(fontStyle);
  const guides = document.createElement('style');
  guides.id = 'mdpdf-guides';
  document.head.append(guides);
  let showGuides = true, reading = false, revision, updating = false, updateId = 0;
  const readStyle = document.createElement('style');
  readStyle.id = 'mdpdf-read';
  document.head.append(readStyle);
  const READ_CSS = `
    html { width:100%; min-height:100%; font:18px/1.75 system-ui,sans-serif; text-align:left; color:var(--zen-text); background:var(--zen-bg); color-scheme:var(--zen-scheme); }
    body { margin:0; padding:0; min-height:100%; background:none; }
    main { width:100%; max-width:calc(70ch + 48px); margin:0 auto; padding:32px 24px 20vh; overflow-wrap:anywhere; }
    h1,h2,h3,h4,h5,h6 { font-family:inherit; color:inherit; line-height:1.3; text-transform:none; }
    h1 { font-size:2em; } h2 { font-size:1.5em; } h3 { font-size:1.25em; } h4,h5,h6 { font-size:1em; }
    a { color:var(--zen-link); } blockquote,.markdown-alert { color:inherit; }
    .mdpdf-read-scroll { width:100%; max-width:100%; overflow-x:auto; }
    table { width:100%; font-size:.9em; table-layout:auto; }
    th,td { white-space:normal; overflow-wrap:normal; border-color:var(--zen-border); }
    th,tbody tr:nth-child(even) { background:var(--zen-raised); color:inherit; }
    pre.mdpdf-code { max-width:100%; overflow-x:auto; white-space:pre; overflow-wrap:normal; font-size:.85em; }
    .mdpdf-math { max-width:100%; overflow-x:auto; }
    .mdpdf-image,.mdpdf-diagram { max-width:100%; background:transparent; }
    img,.mdpdf-diagram svg { max-width:100%; height:auto; }
    .mdpdf-page-break { break-after:auto; height:2rem; }
    .mdpdf-cover { min-height:0; break-after:auto; }
    .mdpdf-cover h1 { font-size:2em; }
    [data-reading-tool]{display:block;margin:8px 0 8px auto;padding:8px 12px;min-height:40px;border:1px solid var(--zen-border);border-radius:5px;color:var(--zen-text);background:var(--zen-raised);cursor:pointer;font:13px system-ui,sans-serif} mark[data-reading-match]{background:#f6df77;color:#171b19} mark.current-match{outline:2px solid #c58c23}
    @media(max-width:599px) { html { font-size:16px; } main { padding:20px 20px 20vh; } }
  `;
  let readingScroll = 0;
  const syncReadingTables = () => {
    if (!reading) {
      for (const wrap of [...document.querySelectorAll('.mdpdf-read-scroll')]) wrap.replaceWith(...wrap.childNodes);
      return;
    }
    for (const table of [...document.querySelectorAll('main table')]) {
      if (table.parentElement?.classList.contains('mdpdf-read-scroll')) continue;
      const wrap = document.createElement('div');
      wrap.className = 'mdpdf-read-scroll';
      table.replaceWith(wrap);
      wrap.append(table);
    }
  };
  const syncReadingTools = () => {
    document.querySelectorAll('[data-reading-tool]').forEach(node=>node.remove());
    if(!reading)return;
    for(const pre of document.querySelectorAll('main pre.mdpdf-code')){
      const copy=document.createElement('button');copy.dataset.readingTool='copy';copy.textContent='Copy code';copy.setAttribute('aria-label','Copy code');copy.onclick=()=>{const clone=pre.cloneNode(true);clone.querySelectorAll('[data-reading-tool],.line-numbers').forEach(node=>node.remove());send({type:'copy-code',text:clone.textContent});};pre.prepend(copy);
    }
    for(const figure of document.querySelectorAll('main .mdpdf-diagram,main .mdpdf-image')){
      const enlarge=document.createElement('button');enlarge.dataset.readingTool='enlarge';enlarge.textContent='Enlarge';enlarge.onclick=()=>{const svg=figure.querySelector('svg'),img=figure.querySelector('img');if(svg)send({type:'enlarge',svg:svg.outerHTML});else if(img)send({type:'enlarge',src:img.src,alt:img.alt});};figure.append(enlarge);
    }
  };
  let found=[],foundIndex=-1;
  function clearFind(){if(!found.length&&!main.querySelector('[data-reading-match]'))return;document.querySelectorAll('[data-reading-match]').forEach(mark=>mark.replaceWith(...mark.childNodes));main.normalize();found=[];foundIndex=-1;}
  function locateMatch(delta){
    if(found.length){foundIndex=(foundIndex+delta+found.length)%found.length;found.forEach((group,i)=>group.forEach(mark=>mark.classList.toggle('current-match',i===foundIndex)));const y=found[foundIndex][0].getBoundingClientRect().top+window.scrollY-60;if(reading)window.scrollTo(0,y);else send({type:'find-position',y});}
    send({type:'find-result',count:found.length,index:foundIndex+1});
  }
  function findText(query){
    clearFind();
    if(query){
      const walker=document.createTreeWalker(main,NodeFilter.SHOW_TEXT),groups=new Map(),ranges=new Map();let node;
      while((node=walker.nextNode())){
        if(node.parentElement.closest('[data-reading-tool],script,style'))continue;
        const block=node.parentElement.closest('p,li,h1,h2,h3,h4,h5,h6,pre,td,th,figcaption')||main;
        if(!groups.has(block))groups.set(block,[]);groups.get(block).push(node);
      }
      const needle=query.toLocaleLowerCase();
      for(const nodes of groups.values()){
        const text=nodes.map(node=>node.textContent).join('').toLocaleLowerCase();let start=0,index;
        while(found.length<500&&(index=text.indexOf(needle,start))!==-1){
          const group=[];found.push(group);let offset=0;
          for(const node of nodes){const end=offset+node.textContent.length;if(end>index&&offset<index+query.length){if(!ranges.has(node))ranges.set(node,[]);ranges.get(node).push({from:Math.max(0,index-offset),to:Math.min(node.textContent.length,index+query.length-offset),group});}offset=end;}
          start=index+query.length;
        }
      }
      for(const [node,segments] of ranges){const fragment=document.createDocumentFragment(),text=node.textContent;let offset=0;
        for(const segment of segments){fragment.append(document.createTextNode(text.slice(offset,segment.from)));const svg=node.parentElement.namespaceURI==='http://www.w3.org/2000/svg',mark=svg?document.createElementNS('http://www.w3.org/2000/svg','tspan'):document.createElement('mark');mark.dataset.readingMatch='';mark.textContent=text.slice(segment.from,segment.to);segment.group.push(mark);fragment.append(mark);offset=segment.to;}
        fragment.append(document.createTextNode(text.slice(offset)));node.replaceWith(fragment);
      }
    }
    locateMatch(1);
  }
  const updateGuides = () => {
    const height = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--page-height')) * 96 / 25.4;
    const css = !reading && showGuides && height ? `@media screen { body { background-image: repeating-linear-gradient(to bottom, transparent 0, transparent ${height - 1}px, #a8b5a780 ${height - 1}px, #a8b5a780 ${height}px); } }` : '';
    if (guides.textContent !== css) guides.textContent = css;
  };
  const setReading = (enabled, dark, request) => {
    const changed = reading !== enabled;
    reading = enabled;
    const palette = dark
      ? '--zen-bg:#171b19;--zen-text:#e7eae5;--zen-link:#b6ed8c;--zen-border:#303631;--zen-raised:#222724;--zen-scheme:dark;'
      : '--zen-bg:#f6f7f3;--zen-text:#263022;--zen-link:#53743b;--zen-border:#dfe4d8;--zen-raised:#eef1e9;--zen-scheme:light;';
    readStyle.textContent = enabled ? `html { ${palette} }\n${READ_CSS}` : '';
    updateGuides(); syncReadingTables(); syncReadingTools(); invalidatePositions();
    if (enabled) {
      // Hidden iframes may suspend animation callbacks. Apply layout before acknowledging.
      void document.documentElement.offsetHeight;
      if (changed) window.scrollTo(0,readingScroll);
      send({type:'read-ready',request});
    } else if (changed) requestAnimationFrame(() => { if (!reading) window.scrollTo(0,0); });
  };
  document.addEventListener('keydown', event => {
    if(event.isComposing)return;
    if((event.ctrlKey||event.metaKey)&&event.shiftKey&&!event.altKey&&event.code==='KeyP'){event.preventDefault();event.stopPropagation();if(!event.repeat)send({type:'tools-command',command:'palette'});return;}
    if (!reading) return;
    if((event.ctrlKey||event.metaKey)&&!event.shiftKey&&!event.altKey&&event.code==='KeyF'){event.preventDefault();event.stopPropagation();send({type:'tools-command',command:'find'});return;}
    let mode;
    if(event.key==='Escape') mode='off';
    else if(event.altKey&&!event.ctrlKey&&!event.metaKey&&!event.shiftKey) {
      if(event.code==='Digit1') mode='read';
      if(event.code==='Digit2') mode='write';
    }
    if(!mode) return;
    event.preventDefault(); event.stopPropagation();
    if(!event.repeat) send(mode==='off' ? {type:'exit-read'} : {type:'zen-mode',mode});
  },true);
  const main = document.querySelector('main');
  // Retain rendered diagrams, decoded images and math for unchanged blocks.
  const originalMarkup = new WeakMap();
  for (const node of main.childNodes) originalMarkup.set(node, node.nodeType === 1 ? node.outerHTML : node.textContent);
  const patchBody = (body, preserveImages) => {
    const images = preserveImages ? new Map([...main.querySelectorAll('.mdpdf-image[data-source-line]')].map(image => [image.dataset.sourceLine, image.cloneNode(true)])) : null;
    const template = document.createElement('template');
    template.innerHTML = body;
    const available = new Map();
    for (const node of main.childNodes) {
      const key = originalMarkup.get(node);
      if (!available.has(key)) available.set(key, []);
      available.get(key).push(node);
    }
    let cursor = main.firstChild;
    for (const next of [...template.content.childNodes]) {
      const key = next.nodeType === 1 ? next.outerHTML : next.textContent;
      const node = available.get(key)?.shift() || next;
      originalMarkup.set(node, key);
      if (node === cursor) cursor = cursor.nextSibling;
      else main.insertBefore(node, cursor);
    }
    while (cursor) { const next = cursor.nextSibling; cursor.remove(); cursor = next; }
    if (images) for (const image of main.querySelectorAll('.mdpdf-image[data-source-line]')) {
      const embedded = images.get(image.dataset.sourceLine);
      if (embedded && !image.querySelector('img[src^="data:"]')) image.replaceWith(embedded);
    }
    invalidatePositions();
  };
  let byLine = [], byPosition = [], positionsDirty = true;
  const invalidatePositions = () => { positionsDirty = true; };
  const indexPositions = () => {
    if (!positionsDirty) return;
    byPosition = [...main.querySelectorAll('[data-source-line]')].map(element => ({ line: Number(element.dataset.sourceLine), y: element.getBoundingClientRect().top + window.scrollY }));
    byLine = [...byPosition].sort((a,b) => a.line - b.line);
    byPosition.sort((a,b) => a.y - b.y);
    positionsDirty = false;
  };
  const atOrBefore = (items, value, field) => {
    let low = 0, high = items.length;
    while (low < high) { const mid = (low + high) >>> 1; if (items[mid][field] <= value) low = mid + 1; else high = mid; }
    return items[Math.max(0, low - 1)];
  };
  let frame, lastSize;
  const report = () => {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => {
      const css = getComputedStyle(document.documentElement);
      const size = { type: 'size', height: Math.max(document.body.scrollHeight, document.body.offsetHeight), width: parseFloat(css.getPropertyValue('--page-width')) * 96 / 25.4, pageHeight: parseFloat(css.getPropertyValue('--page-height')) * 96 / 25.4 };
      const key = JSON.stringify(size);
      if (key !== lastSize) { lastSize = key; send(size); }
      if (!updating) indexPositions();
    });
  };
  const rendered = () => { if(reading)syncReadingTools(); invalidatePositions(); updateGuides(); report(); send({ type: 'ready', revision, diagnostics: window.__MDPDF_ERRORS__ || [] }); };
  window.addEventListener('mdpdf-rendered', () => { if (!updating) rendered(); });
  if (window.__MDPDF_READY__) rendered();
  new ResizeObserver(() => { invalidatePositions(); report(); }).observe(document.body);
  document.addEventListener('load', () => { invalidatePositions(); report(); }, true);
  let scrollFrame, pendingScroll;
  const syncScroll = message => {
    pendingScroll = message;
    if (scrollFrame) return;
    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = null;
      indexPositions();
      const message = pendingScroll;
      if (reading) return;
      if (message.type === 'scroll-line') {
        const target = atOrBefore(byLine, message.line, 'line');
        if (target) send({ type: 'scroll-position', y: target.y });
      } else {
        const target = atOrBefore(byPosition, message.y + 30, 'y');
        send({ type: 'editor-line', line: target?.line || 1 });
      }
    });
  };
  let mathLoading;
  window.addEventListener('message', async event => {
    if (event.source !== window.parent || !event.data?.mdpdf) return;
    const message = event.data;
    if(message.type==='find-text'){findText(String(message.query||'').slice(0,500));return;}
    if(message.type==='find-next'){locateMatch(message.delta===-1?-1:1);return;}
    if(message.type==='preflight'){
      const items=[],seen=new Set(),bounds=main.getBoundingClientRect();
      const add=(element,text,severity='Warning')=>{const line=Number(element.closest('[data-source-line]')?.dataset.sourceLine||0),key=line+text;if(seen.has(key))return;seen.add(key);items.push({severity,message:text,source_line:line||null});};
      for(const img of main.querySelectorAll('img'))if(!img.complete||!img.naturalWidth)add(img,'Image could not be loaded.','Error');
      for(const box of main.querySelectorAll('.mdpdf-error'))add(box,box.textContent.slice(0,500),'Error');
      for(const block of main.querySelectorAll('pre.mdpdf-code,table,.mdpdf-math,.mdpdf-diagram,img')){const rect=block.getBoundingClientRect();if(block.scrollWidth>block.clientWidth+2||rect.right>bounds.right+2||rect.left<bounds.left-2)add(block,'Content may overflow or be clipped in the PDF.');}
      send({type:'preflight-result',request:message.request,items});return;
    }
    if (message.type === 'invalidate') { revision = message.revision; updating = true; window.__MDPDF_CANCEL__?.(); return; }
    if (message.type === 'style') {
      clearFind();
      if (message.revision !== undefined && revision !== undefined && message.revision < revision) return;
      revision = message.revision ?? revision;
      const current = ++updateId;
      updating = true;
      window.__MDPDF_INTERACTIVE__ = true;
      if (message.metadata) { document.documentElement.lang = message.metadata.lang; document.title = message.metadata.title; }
      const newFaces = message.css.match(/@font-face\s*\{[^}]*\}/g) || [];
      const count = fontFaces.size;
      for (const face of newFaces) fontFaces.add(face);
      if (count !== fontFaces.size) fontStyle.textContent = [...fontFaces].join('\n');
      const css = message.css.replace(/@font-face\s*\{[^}]*\}/g, '');
      if (style.textContent !== css) style.textContent = css;
      if (Array.isArray(message.fonts)) {
        let dynamicFonts = document.getElementById('mdpdf-custom-fonts');
        if (!dynamicFonts) { dynamicFonts = document.createElement('style'); dynamicFonts.id = 'mdpdf-custom-fonts'; document.head.append(dynamicFonts); }
        const css = message.fonts.filter(font => /^assetfont-[a-f0-9]{64}$/.test(font.family) && /^data:font\/(?:woff2|ttf|otf);base64,[A-Za-z0-9+/=]+$/.test(font.url)).map(font => `@font-face{font-family:"${font.family}";src:url(${font.url});font-weight:100 900;font-style:normal;font-display:block;}`).join('\n');
        if (dynamicFonts.textContent !== css) dynamicFonts.textContent = css;
      }
      if (typeof message.body === 'string') patchBody(message.body, message.preserveImages);
      if (reading) syncReadingTables();
      invalidatePositions();
      updateGuides();
      send({ type: 'content', revision });
      if (message.deferRender) { report(); return; }
      try {
        if (!window.katex && /^data:text\/javascript;base64,[A-Za-z0-9+/=]+$/.test(message.mathScript || '')) {
          mathLoading ||= new Promise((resolve,reject) => {
            const script = document.createElement('script');
            script.nonce = document.querySelector('script[nonce]').nonce;
            script.src = message.mathScript;
            script.onload = resolve; script.onerror = () => reject(new Error('Math runtime could not load'));
            document.body.append(script);
          });
        }
        if (mathLoading) await mathLoading;
        if (current !== updateId) return;
        await window.__MDPDF_RENDER__(message.config);
      } catch (error) { send({ type: 'bridge-error', message: String(error) }); }
      if (current === updateId && (message.revision === undefined || message.revision === revision)) { updating = false; rendered(); }
    } else if (message.type === 'guides') { showGuides = message.enabled; updateGuides(); }
    else if (message.type === 'suspend-read') { if (reading) readingScroll = window.scrollY; send({type:'read-suspended'}); }
    else if (message.type === 'read') setReading(Boolean(message.enabled),Boolean(message.dark),message.request);
    else if (message.type === 'read-line' && reading) { indexPositions();const target=atOrBefore(byLine,message.line,'line');if(target)window.scrollTo(0,target.y); }
    else if (message.type === 'scroll-line' || message.type === 'position-line') syncScroll(message);
  });
})();
