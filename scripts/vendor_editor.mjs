import { build } from 'esbuild';
import { mkdirSync, readFileSync, writeFileSync, copyFileSync, cpSync, readdirSync } from 'node:fs';
import { createHash } from 'node:crypto';
mkdirSync('public/vendor', { recursive: true });
mkdirSync('assets/vendor/codemirror', { recursive: true });
await build({ stdin: { contents: `export {EditorView, keymap, lineNumbers, highlightActiveLineGutter, drawSelection, highlightActiveLine, ViewPlugin, Decoration} from '@codemirror/view';
export {EditorState, Compartment, StateEffect, StateField} from '@codemirror/state';
export {basicSetup} from 'codemirror';
export {markdown, markdownLanguage} from '@codemirror/lang-markdown';
export {css} from '@codemirror/lang-css';
export {oneDark} from '@codemirror/theme-one-dark';
export {setDiagnostics} from '@codemirror/lint';
export {openSearchPanel} from '@codemirror/search';
`, resolveDir: process.cwd(), sourcefile: 'editor-entry.js' }, outfile: 'assets/vendor/codemirror/editor.js', bundle: true, format: 'esm', minify: true, legalComments: 'eof', target: 'es2022' });
copyFileSync('assets/vendor/codemirror/editor.js', 'public/vendor/editor.js');
const packages = ['codemirror', ...Object.keys(JSON.parse(readFileSync('package-lock.json')).packages).filter(x => x.startsWith('node_modules/@codemirror/') || x.startsWith('node_modules/@lezer/')).map(x => x.slice('node_modules/'.length)), 'style-mod', 'w3c-keyname', 'crelt'];
const licenses = [];
for (const name of new Set(packages)) {
  const pkg = JSON.parse(readFileSync(`node_modules/${name}/package.json`));
  let text;
  for (const file of ['LICENSE', 'LICENSE.txt']) { try { text = readFileSync(`node_modules/${name}/${file}`, 'utf8'); break; } catch {} }
  if (!text) throw new Error('Missing license: ' + name);
  licenses.push(`\n${name} ${pkg.version}\n${text}`);
}
writeFileSync('assets/vendor/codemirror/LICENSES.txt', licenses.join('\n'));
copyFileSync('assets/vendor/codemirror/LICENSES.txt', 'public/vendor/LICENSES.txt');
writeFileSync('assets/vendor/codemirror/manifest.json', JSON.stringify({ codemirror: '6.0.2', sha256: createHash('sha256').update(readFileSync('assets/vendor/codemirror/editor.js')).digest('hex'), dependency_lock: 'package-lock.json' }, null, 2) + '\n');
for (const [source, target] of [['inter/inter-latin-wght-normal.woff2','Inter.woff2'],['jetbrains-mono/jetbrains-mono-latin-wght-normal.woff2','JetBrainsMono.woff2']]) copyFileSync('assets/fonts/' + source, 'public/ui-fonts/' + target);
console.log('Vendored CodeMirror and its exact dependency licenses.');

cpSync('assets/fonts', 'public/fonts', { recursive: true });
writeFileSync('public/fonts/fonts.css', readFileSync('assets/fonts/fonts.css', 'utf8').replaceAll('/assets/fonts/', '/fonts/'));

mkdirSync('assets/vendor/katex/fonts', {recursive:true});
copyFileSync('node_modules/katex/dist/katex.min.js','assets/vendor/katex/katex.min.js');
copyFileSync('node_modules/katex/LICENSE','assets/vendor/katex/LICENSE');
const katexCss=readFileSync('node_modules/katex/dist/katex.min.css','utf8').replace(/src:url\((fonts\/[^)]+\.woff2)\)[^;}]+/g,(_,file)=>`src:url(/assets/vendor/katex/${file}) format("woff2")`);
writeFileSync('assets/vendor/katex/katex.min.css',katexCss);
for(const match of katexCss.matchAll(/url\(\/assets\/vendor\/katex\/(fonts\/[^)]+)\)/g))copyFileSync('node_modules/katex/dist/'+match[1],'assets/vendor/katex/'+match[1]);
const katexFiles=['katex.min.js','katex.min.css','LICENSE',...readdirSync('assets/vendor/katex/fonts').map(x=>'fonts/'+x)];
writeFileSync('assets/vendor/katex/manifest.json',JSON.stringify({version:'0.19.0',source:'https://www.npmjs.com/package/katex/v/0.19.0',files:Object.fromEntries(katexFiles.map(file=>[file,createHash('sha256').update(readFileSync('assets/vendor/katex/'+file)).digest('hex')]))},null,2)+'\n');
