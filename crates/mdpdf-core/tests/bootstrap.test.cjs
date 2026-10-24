const { test } = require('node:test');
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { resolve } = require('node:path');
const vm = require('node:vm');
const bootstrap = readFileSync(resolve(__dirname, '../../../assets/vendor/mermaid/bootstrap.js'), 'utf8');

function environment(count, failFirst = false) {
  let resolveFonts;
  let fontsReady = false;
  let initialized = false;
  const calls = [];
  const elements = Array.from({ length: count }, (_, index) => ({
    textContent: `flowchart LR\n A${index}-->B${index}`,
    dataset: { diagramId: String(index) }, style: {}, classList: { add() {} }, removeAttribute() {},
    closest: () => ({ dataset: { sourceLine: String(index + 1) } }),
    svg: { attributes: {}, style: {},
      getBBox: () => ({ x: 0, y: 0, width: 200, height: 100 }),
      viewBox: { baseVal: { x: 0, y: 0, width: 100, height: 100 } },
      setAttribute(name, value) { this.attributes[name] = value; },
    },
    querySelector() { return this.svg; },
    replaceWith(box) { this.errorBox = box; },
    replaceChildren(box) { this.errorBox = box; },
  }));
  const document = {
    fonts: { load: async () => { assert.ok(initialized, 'Mermaid autorun must be disabled before awaiting fonts'); }, ready: new Promise(resolve => {
      resolveFonts = () => { fontsReady = true; resolve(); };
    }) },
    getElementById: () => ({ textContent: JSON.stringify({ securityLevel: 'strict', startOnLoad: false }) }),
    querySelectorAll: selector => selector.startsWith('pre') ? elements : [],
    querySelector: () => elements[0],
    createElement: tag => ({ tag, children: [], append(...children) { this.children.push(...children); } }),
  };
  const mermaid = {
    initialize(config) {
      initialized = true;
      assert.equal(config.securityLevel, 'strict');
      assert.equal(config.startOnLoad, false);
    },
    async render(id, source, node) {
      const nodes = [node];
      assert.ok(fontsReady, 'Mermaid ran before fonts');
      calls.push(nodes[0]);
      if (failFirst && nodes[0] === elements[0]) throw new Error('Parse error on line 2');
      return {svg: '<svg></svg>'};
    },
  };
  const context = { document, mermaid, performance, setTimeout, clearTimeout, Event: class { constructor(type) { this.type = type; } }, dispatchEvent() {} };
  context.window = context;
  return { context, elements, calls, resolveFonts };
}

test('fonts settle before Mermaid and the final ready flag', async () => {
  const env = environment(1);
  const done = vm.runInNewContext(bootstrap, env.context);
  assert.equal(env.calls.length, 0);
  assert.equal(env.context.__MDPDF_READY__, false);
  env.resolveFonts();
  await done;
  assert.equal(env.calls.length, 1);
  assert.equal(env.context.__MDPDF_READY__, true);
  assert.equal(env.context.__MDPDF_ERRORS__.length, 0);
  assert.equal(env.elements[0].svg.attributes.viewBox, '-4 -4 208 108');
  assert.equal(env.elements[0].svg.style.maxWidth, 'min(100%, 208px)');
});

test('one diagram error produces safe text and does not stop later diagrams', async () => {
  const env = environment(2, true);
  const done = vm.runInNewContext(bootstrap, env.context);
  env.resolveFonts();
  await done;
  assert.equal(env.calls.length, 2);
  assert.equal(env.context.__MDPDF_READY__, true);
  assert.equal(env.context.__MDPDF_ERRORS__.length, 1);
  assert.equal(env.context.__MDPDF_ERRORS__[0].source_line, 1);
  const box = env.elements[0].errorBox;
  assert.equal(box.className, 'mdpdf-error');
  assert.ok(box.children[0].textContent.includes('Parse error on line 2'));
  assert.equal(box.children[1].children[1].textContent, env.elements[0].textContent);
});
