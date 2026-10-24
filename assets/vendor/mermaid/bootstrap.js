(async () => {
  'use strict';
  window.__MDPDF_READY__ = false;
  window.__MDPDF_ERRORS__ = [];
  const sources = new WeakMap();
  const cache = new Map();
  const embeddedCache = document.getElementById('mdpdf-cached-svgs');
  if (embeddedCache?.tagName === 'SCRIPT' && embeddedCache.type === 'application/json' && embeddedCache.nonce === document.currentScript?.nonce) {
    try { for (const [key, value] of JSON.parse(embeddedCache.textContent)) cache.set(key, value); } catch {}
  }
  let generation = 0;
  const report = (element, error, source) => {
    const line = Number(element.closest('[data-source-line]')?.dataset.sourceLine || 0);
    const message = String(error?.message || error);
    window.__MDPDF_ERRORS__.push({ severity: 'Error', kind: 'Mermaid', message, source_line: line || null });
    const box = document.createElement('div');
    box.className = 'mdpdf-error';
    const label = document.createElement('p');
    label.textContent = `Diagram error at source line ${line}: ${message}`;
    const details = document.createElement('details');
    const summary = document.createElement('summary');
    summary.textContent = 'Diagram source';
    const pre = document.createElement('pre');
    pre.textContent = source;
    details.append(summary, pre);
    box.append(label, details);
    element.style.whiteSpace = 'normal';
    element.replaceChildren(box);
  };
  const timeout = (promise, milliseconds) => {
    let timer;
    return Promise.race([promise, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error('Diagram rendering timed out')), milliseconds);
    })]).finally(() => clearTimeout(timer));
  };
  const hash = text => {
    let value = 2166136261;
    for (let index = 0; index < text.length; index++) value = Math.imul(value ^ text.charCodeAt(index), 16777619);
    return (value >>> 0).toString(36);
  };
  const render = async config => {
    const current = ++generation;
    window.__MDPDF_READY__ = false;
    window.__MDPDF_ERRORS__ = [];
    try {
      const diagrams = [...document.querySelectorAll('pre[data-diagram-id]')];
      const families = new Set([config.fontFamily, 'Noto Sans', 'Noto Sans CJK SC']);
      const fallback = `${String(config.fontFamily).replace(/[^\p{L}\p{N} _.-]/gu, "")}, Noto Sans, Noto Sans CJK SC, sans-serif`;
      const runtimeConfig = { ...config, fontFamily: fallback, themeVariables: { ...config.themeVariables, fontFamily: fallback } };
      for (const element of diagrams) if (!sources.has(element)) sources.set(element, element.textContent);
      if (diagrams.length && !window.mermaid) throw new Error('Vendored Mermaid 11.17.2 is unavailable');
      // Prevent Mermaid's DOMContentLoaded autorun from racing font loading.
      if (window.mermaid) mermaid.initialize({ ...runtimeConfig, securityLevel: 'strict', startOnLoad: false });
      if (diagrams.length) {
        const text = diagrams.map(element => sources.get(element)).join('\n');
        await timeout(Promise.all([...families].map(family => document.fonts.load(`${config.fontSize}px ${JSON.stringify(family)}`, text))), 5000);
      }
      for (const element of document.querySelectorAll('.mdpdf-math')) {
        if (element.dataset.rendered) continue;
        const source = element.textContent;
        if (window.katex) {
          katex.render(source, element, {displayMode: element.dataset.display === 'true', throwOnError: false, trust: false, strict: 'warn', maxExpand: 1000, maxSize: 20, output: 'htmlAndMathml'});
          element.dataset.rendered = 'true';
        }
      }
      await document.fonts.ready;
      for (const element of document.querySelectorAll('[data-width]')) {
        const width = Number(element.dataset.width);
        if (width > 0 && width <= 100) element.style.width = `${width}%`;
      }
      for (const element of document.querySelectorAll('[data-align]')) {
        const align = element.dataset.align;
        if (!['left', 'center', 'right'].includes(align)) continue;
        element.style.textAlign = align;
        element.style.marginLeft = align === 'left' ? '0' : 'auto';
        element.style.marginRight = align === 'right' ? '0' : 'auto';
      }
      const deadline = performance.now() + 30000;
      let yieldedAt = performance.now();
      for (const element of diagrams.slice(0, 200)) {
        if (current !== generation) return;
        const source = sources.get(element);
        try {
          const remaining = deadline - performance.now();
          if (remaining <= 0) throw new Error('Document rendering timed out');
          const localTheme = element.closest('[data-mermaid-theme]')?.dataset.mermaidTheme;
          const key = hash(JSON.stringify([source, config, localTheme, element.dataset.diagramId, Math.round(element.parentElement?.clientWidth || 0)]));
          if (element.dataset.mdpdfKey === key && element.querySelector('svg')) continue;
          element.style.whiteSpace = '';
          element.removeAttribute('data-processed');
          if (cache.has(key)) { element.innerHTML = cache.get(key); element.dataset.mdpdfKey = key; continue; }
          element.textContent = source;
          element.classList.add('mermaid');
          mermaid.initialize({ ...runtimeConfig, ...(localTheme ? { theme: localTheme } : {}), deterministicIds: true, deterministicIDSeed: key, securityLevel: 'strict', startOnLoad: false });
          // Mermaid 11 seeds run() IDs from seed.length, so independent runs
          // collide even with different hashes. Supply an explicit stable ID.
          const originalRandom = Math.random;
          let randomState = parseInt(hash(source), 36) || 1;
          Math.random = () => {
            randomState ^= randomState << 13; randomState ^= randomState >>> 17; randomState ^= randomState << 5;
            return (randomState >>> 0) / 4294967296;
          };
          try {
            const result = await timeout(mermaid.render(`mdpdf-svg-${element.dataset.diagramId}-${key}`, source, element), Math.min(5000, remaining));
            element.innerHTML = result.svg;
          } finally { Math.random = originalRandom; }
          const svg = element.querySelector('svg');
          if (!svg) throw new Error('Mermaid did not produce SVG');
          const bounds = svg.getBBox();
          const view = svg.viewBox.baseVal;
          if (view.width > 0 && view.height > 0 && (bounds.x < view.x || bounds.y < view.y || bounds.x + bounds.width > view.x + view.width || bounds.y + bounds.height > view.y + view.height)) {
            const left = Math.min(view.x, bounds.x - 4), top = Math.min(view.y, bounds.y - 4);
            const right = Math.max(view.x + view.width, bounds.x + bounds.width + 4), bottom = Math.max(view.y + view.height, bounds.y + bounds.height + 4);
            svg.setAttribute('viewBox', `${left} ${top} ${right - left} ${bottom - top}`);
            svg.setAttribute('width', '100%');
            svg.style.maxWidth = `min(100%, ${right - left}px)`;
          }
          svg.setAttribute('role', 'img');
          element.dataset.mdpdfKey = key;
          cache.set(key, svg.outerHTML);
          if (cache.size > 200) cache.delete(cache.keys().next().value);
        } catch (error) { report(element, error, source); }
        // Give input, scrolling and paint a turn between expensive diagrams in
        // the live editor. Standalone PDF exports keep the eager render path.
        if (window.__MDPDF_INTERACTIVE__ && performance.now() - yieldedAt > 8) {
          await new Promise(resolve => setTimeout(resolve, 0));
          yieldedAt = performance.now();
        }
      }
    } catch (error) {
      const element = document.querySelector('pre[data-diagram-id]') || document.querySelector('main');
      report(element, error, element.textContent);
    } finally {
      if (current === generation) {
        window.__MDPDF_READY__ = true;
        window.__MDPDF_CACHE__ = [...cache];
        window.dispatchEvent(new Event('mdpdf-rendered'));
      }
    }
  };
  // Coalesce rapid edits while preserving Mermaid's single-render ownership.
  let pendingConfig, pending = Promise.resolve(), running = false;
  window.__MDPDF_CANCEL__ = () => {
    generation++;
    pendingConfig = null;
    window.__MDPDF_READY__ = false;
  };
  window.__MDPDF_RENDER__ = config => {
    pendingConfig = config;
    generation++;
    if (!running) {
      running = true;
      pending = (async () => {
        try { while (pendingConfig) { const next = pendingConfig; pendingConfig = null; await render(next); } }
        finally { running = false; }
      })();
    }
    return pending;
  };
  const config = JSON.parse(document.getElementById('mdpdf-config').textContent);
  await window.__MDPDF_RENDER__(config);
})();
