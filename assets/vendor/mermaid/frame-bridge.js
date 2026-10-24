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
  let showGuides = true, revision, updating = false, updateId = 0;
  const updateGuides = () => {
    const height = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--page-height')) * 96 / 25.4;
    const css = showGuides && height ? `@media screen { body { background-image: repeating-linear-gradient(to bottom, transparent 0, transparent ${height - 1}px, #a8b5a780 ${height - 1}px, #a8b5a780 ${height}px); } }` : '';
    if (guides.textContent !== css) guides.textContent = css;
  };
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
  const rendered = () => { invalidatePositions(); updateGuides(); report(); send({ type: 'ready', revision, diagnostics: window.__MDPDF_ERRORS__ || [] }); };
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
    if (message.type === 'invalidate') { revision = message.revision; updating = true; window.__MDPDF_CANCEL__?.(); return; }
    if (message.type === 'style') {
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
    } else if (message.type === 'print') {
      if (message.revision !== revision || updating || !window.__MDPDF_READY__) { send({type:'bridge-error',message:'Wait for the preview to finish before printing'}); return; }
      document.title = message.title || document.title;
      window.focus(); window.print();
      send({type:'printed'});
    } else if (message.type === 'guides') { showGuides = message.enabled; updateGuides(); }
    else if (message.type === 'scroll-line' || message.type === 'position-line') syncScroll(message);
  });
})();
