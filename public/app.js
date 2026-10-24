import { EditorView, EditorState, Compartment, basicSetup, markdown, oneDark, setDiagnostics, openSearchPanel } from './vendor/editor.js';
import { settings } from './settings.js';
import {LocalAssets,assetUrl} from './browser-assets.js';
import {BrowserRenderer} from './browser-renderer.js';
const $ = id => document.getElementById(id);
const DRAFT_KEY = 'mdpdf:draft:v1', THEMES_KEY = 'mdpdf:themes:v1';
const SAMPLE = `# A better way to share ideas

Great documents make complex ideas feel simple.
This is your space to write, refine, and turn a little
Markdown into something worth sharing.

## Made for the details

From a quick project brief to a technical deep-dive,
every page should feel considered. Start with your
words. We'll take care of the layout.

> [!TIP]
> Make it yours. Try a theme, adjust the type, or drop
> in a diagram. Your preview updates as you write.

## A clear plan

| What we're building | Status | Owner |
| :--- | :---: | --- |
| A focused writing space | Complete | Design |
| Beautiful, vector diagrams | Complete | Engineering |
| Documents ready to share | In progress | You |

## From first thought to final page

\`\`\`mermaid {caption="A simpler publishing workflow"}
flowchart LR
  A[Write] --> B[Refine]
  B --> C[Share]
\`\`\`

### The little things add up

- **Write naturally.** All the Markdown you know.
- **Keep it consistent.** One theme, every page.
- **Take it anywhere.** A print-ready PDF in one click.

---

*Made with care. Written by you.*
`;
const SNIPPETS = {
  table:'\n| Column | Column |\n| --- | --- |\n| Value | Value |\n', flowchart:'\n```mermaid\nflowchart LR\n  A[Start] --> B[Finish]\n```\n', sequence:'\n```mermaid\nsequenceDiagram\n  Alice->>Bob: Hello\n  Bob-->>Alice: Hi\n```\n', er:'\n```mermaid\nerDiagram\n  USER ||--o{ DOCUMENT : writes\n```\n', gantt:'\n```mermaid\ngantt\n  dateFormat YYYY-MM-DD\n  section Project\n  Write :a1, 2026-10-01, 3d\n  Share :after a1, 2d\n```\n', pagebreak:'\n<!-- pagebreak -->\n', toc:'\n[[toc]]\n',
};
let started = false;
function core(action,payload='') {
  const raw = window.mdpdfCore(action,typeof payload === 'string' ? payload : JSON.stringify(payload));
  if (action === 'export' && !raw.startsWith('{"error"')) return raw;
  const parsed = JSON.parse(raw); if (parsed.error) throw new Error(parsed.error); return parsed;
}
function toast(message) { $('toast').textContent = message; $('toast').classList.remove('hidden'); clearTimeout(toast.timer); toast.timer = setTimeout(() => $('toast').classList.add('hidden'),5500); }
function download(blob,name) { const url = URL.createObjectURL(blob); const link = document.createElement('a'); link.href = url; link.download = name; link.click(); setTimeout(() => URL.revokeObjectURL(url),10000); }
function jsonRead(key,fallback) { try { return JSON.parse(localStorage.getItem(key)) || fallback; } catch { return fallback; } }
async function mount() {
  if (started || !window.mdpdfCore) return; started = true;
  const assets = new LocalAssets();
  const storedAssets = await assets.open();
  const saved = jsonRead(DRAFT_KEY,{});
  let theme;
  try { theme = saved.theme || core('default'); core('compile',{markdown:saved.markdown || '',theme}); } catch { theme = core('default'); }
  const state = { markdown:typeof saved.markdown === 'string' ? saved.markdown : SAMPLE, theme, preset:saved.preset || 'default', fonts:storedAssets.filter(asset=>asset.kind==='font'), mode:'live', dark:saved.dark !== false, guides:saved.guides !== false, sync:true, zoom:saved.zoom || 'fit', html:'', diagnostics:[], cache:new Map(), pageWidth:794, pageHeight:1123, height:1123, scale:1, pdfUrl:null, generation:0, savedThemes:jsonRead(THEMES_KEY,[]), pendingStyle:null };
  window.mdpdf = { state, editor:null, ready:false };
  document.documentElement.classList.toggle('light-ui',!state.dark);
  const fontCss = document.createElement('link'); fontCss.rel = 'stylesheet'; fontCss.href = assetUrl('fonts/fonts.css'); document.head.append(fontCss);
  $('filename').value = saved.filename || 'project-brief.md'; $('zoom').value = state.zoom;
  const editorTheme = new Compartment();
  let typingTimer, saveTimer, previewController, muteEditor = 0, mutePreview = 0;
  let frameReady = false, scrollFrame;
  const renderer = new BrowserRenderer(assets,()=>schedulePreview());
  const editor = new EditorView({ parent:$('editor'), state:EditorState.create({doc:state.markdown,extensions:[basicSetup,markdown(),EditorView.lineWrapping,editorTheme.of(state.dark ? oneDark : []),EditorView.contentAttributes.of({'aria-label':'Markdown source','spellcheck':'false'}),EditorView.updateListener.of(update => {
    if (update.docChanged) { state.markdown = update.state.doc.toString(); stats(); schedulePreview(); scheduleSave(); }
    if (update.selectionSet || update.viewportChanged) {
      $('cursor-line').textContent = String(update.state.doc.lineAt(update.state.selection.main.head).number);
    }
  })]}) });
  window.mdpdf.editor = editor;
  function post(message) { $('live-preview').contentWindow?.postMessage({mdpdf:true,...message},'*'); }
  editor.scrollDOM.addEventListener('scroll',() => {
    if (!state.sync || state.mode !== 'live' || performance.now() <= muteEditor) return;
    cancelAnimationFrame(scrollFrame);
    scrollFrame = requestAnimationFrame(() => {
      if (performance.now() <= muteEditor) return;
      const block = editor.lineBlockAtHeight(editor.scrollDOM.scrollTop);
      post({type:'scroll-line',line:editor.state.doc.lineAt(block.from).number});
    });
  },{passive:true});
  function stats() { const words = state.markdown.trim().split(/\s+/).filter(Boolean).length; $('document-stats').textContent = `${words} words  ·  ${editor.state.doc.lines} lines`; }
  function save() {
    try { localStorage.setItem(DRAFT_KEY,JSON.stringify({markdown:state.markdown,theme:state.theme,preset:state.preset,filename:$('filename').value,dark:state.dark,guides:state.guides,zoom:state.zoom})); $('save-state').textContent = '✓ Saved locally'; }
    catch { $('save-state').textContent = 'Storage unavailable'; }
  }
  function scheduleSave() { $('save-state').textContent = 'Saving…'; clearTimeout(saveTimer); saveTimer = setTimeout(save,2000); }
  function status(text,busy=false) { $('render-status').textContent = text; $('render-status').classList.toggle('spinner',busy); }
  function diagnostics(items) {
    state.diagnostics = items;
    const panel = $('diagnostics'); panel.replaceChildren(); panel.classList.toggle('hidden',items.length === 0);
    const markers = [];
    for (const diagnostic of items) {
      const row = document.createElement('button'); row.textContent = `${diagnostic.source_line ? 'Line '+diagnostic.source_line+' · ' : ''}${diagnostic.message}`;
      row.onclick = () => { const line = editor.state.doc.line(Math.min(Math.max(diagnostic.source_line || 1,1),editor.state.doc.lines)); editor.dispatch({selection:{anchor:line.from},effects:EditorView.scrollIntoView(line.from,{y:'center'})}); editor.focus(); };
      panel.append(row);
      if (diagnostic.source_line) { const line = editor.state.doc.line(Math.min(Math.max(diagnostic.source_line,1),editor.state.doc.lines)); markers.push({from:line.from,to:line.to,severity:diagnostic.severity === 'Error' ? 'error':'warning',message:diagnostic.message}); }
    }
    editor.dispatch(setDiagnostics(editor.state,markers));
  }
  function resizePaper() {
    const viewport = $('preview-scroll');
    const scale = state.zoom === 'fit' ? Math.min(1.4,Math.max(.2,(viewport.clientWidth - (window.innerWidth < 900 ? 28:50))/state.pageWidth)) : Number(state.zoom);
    state.scale = scale;
    const frame = $('live-preview'), stage = $('paper-stage');
    frame.style.width = `${state.pageWidth}px`; frame.style.height = `${Math.max(state.height,state.pageHeight)}px`; frame.style.transform = `scale(${scale})`;
    stage.style.width = `${state.pageWidth * scale}px`; stage.style.minHeight = '0'; stage.style.height = `${Math.max(state.height,state.pageHeight)*scale}px`;
  }
  const resizeObserver = new ResizeObserver(resizePaper); resizeObserver.observe($('preview-scroll'));
  function compileTheme(structural=false) {
    const result=core('compile',{markdown:state.markdown,theme:state.theme});
    if(structural)result.body=core('body',{markdown:state.markdown,theme:state.theme}).body;
    result.fonts=documentFonts(result);
    state.pendingStyle={...result,revision:state.revision,preserveImages:structural};
    post({type:'style',...state.pendingStyle});
    pageStatus(result);
    schedulePreview();
    return result;
  }
  function pageStatus(result) {
    const size = result.theme.page.size;
    $('page-status').textContent = `${typeof size === 'string' ? size.toUpperCase() : 'Custom'} · ${result.theme.page.orientation === 'landscape' ? 'Landscape':'Portrait'}`;
  }
  function schedulePreview() {
    clearTimeout(typingTimer); previewController?.abort();
    window.mdpdf.ready=false; $('export-pdf').disabled=true;
    typingTimer=setTimeout(renderPreview,100);
  }
  function documentFonts(result) {
    const names = new Set([result.theme.typography.body_font,result.theme.typography.heading_font,result.theme.typography.mono_font,result.theme.diagrams.font_family]);
    return state.fonts.filter(asset=>asset.dataUrl && names.has('assetfont-'+asset.id)).map(asset=>({family:'assetfont-'+asset.id,url:asset.dataUrl}));
  }
  async function renderPreview() {
    clearTimeout(typingTimer); typingTimer=null;
    previewController?.abort(); previewController=new AbortController();
    const generation=++state.generation, {signal}=previewController;
    state.revision=generation; window.mdpdf.ready=false; window.mdpdf.lastContentMs=null;
    $('export-pdf').disabled=true;
    post({type:'invalidate',revision:generation});
    status('Updating preview',true); state.requestStarted=performance.now();
    try {
      const result=await renderer.render(state.markdown,structuredClone(state.theme),{includeShell:!frameReady,signal});
      if(signal.aborted || generation!==state.generation)return;
      state.renderDiagnostics=result.diagnostics;
      diagnostics(result.diagnostics);
      state.pendingStyle={...result,revision:generation};
      pageStatus(result);
      if(!frameReady){state.html=result.html;$('live-preview').srcdoc=result.html;}
      else post({type:'style',...state.pendingStyle});
    }catch(error){
      if(error.name!=='AbortError'){status('Preview needs attention');toast(error.message);}
    }
  }
  $('live-preview').addEventListener('load',() => { if (state.pendingStyle) post({type:'style',...state.pendingStyle,revision:state.revision}); post({type:'guides',enabled:state.guides}); });
  window.addEventListener('message',event => {
    if (event.source !== $('live-preview').contentWindow || !event.data?.mdpdf) return;
    const message = event.data;
    if (message.type === 'content' && message.revision === state.revision) {
      $('preview-empty').classList.add('hidden');
      window.mdpdf.lastContentMs ??= performance.now() - (state.requestStarted || performance.now());
    }
    if (message.type === 'size') { state.height = message.height; state.pageWidth = message.width || 794; state.pageHeight = message.pageHeight || 1123; resizePaper(); }
    if (message.type === 'ready') {
      if (typingTimer || (message.revision !== undefined && message.revision !== state.revision)) return;
      frameReady = true;
      diagnostics([...(state.renderDiagnostics || []),...message.diagnostics]);
      for (const [key,svg] of message.cache || []) state.cache.set(key,svg);
      while (state.cache.size > 200) state.cache.delete(state.cache.keys().next().value);
      $('preview-empty').classList.add('hidden');
      const count = (state.markdown.match(/^```mermaid/gm) || []).length;
      status(`${message.diagnostics.length ? 'Needs attention':'Preview ready'}${count ? ` · ${count} diagram${count === 1 ? '':'s'}`:''}`);
      window.mdpdf.ready = true; $('export-pdf').disabled=false; window.mdpdf.lastRenderMs = performance.now() - (state.requestStarted || performance.now());
    }
    if (message.type === 'bridge-error') toast(message.message);
    if (message.type === 'printed') status('Preview ready');
    if (message.type === 'open-link') {
      try { const url=new URL(message.url,location.href);if(['http:','https:','mailto:'].includes(url.protocol))window.open(url.href,'_blank','noopener,noreferrer'); } catch {}
    }
    if (message.type === 'scroll-position' && state.sync) { mutePreview = performance.now()+250; $('preview-scroll').scrollTop = Math.max(0,message.y*state.scale-35); }
    if (message.type === 'editor-line' && state.sync) { muteEditor = performance.now()+250; const line = editor.state.doc.line(Math.min(Math.max(message.line,1),editor.state.doc.lines)); editor.dispatch({effects:EditorView.scrollIntoView(line.from,{y:'start'})}); }
  });
  let previewScrollFrame;
  $('preview-scroll').addEventListener('scroll',() => { if (state.sync && state.mode === 'live' && performance.now() > mutePreview) { cancelAnimationFrame(previewScrollFrame); previewScrollFrame=requestAnimationFrame(()=>{if(performance.now()<=mutePreview)return;muteEditor=performance.now()+100;post({type:'position-line',y:$('preview-scroll').scrollTop/state.scale});}); } },{passive:true});
  function presets() {
    const select = $('preset');
    for (const node of select.querySelectorAll('[data-custom]')) node.remove();
    for (const saved of state.savedThemes) { const option = document.createElement('option'); option.value = saved.id; option.textContent = saved.name; option.dataset.custom = 'true'; select.append(option); }
    select.value = state.preset;
    if (!select.value) { state.preset = 'default'; select.value = 'default'; }
  }
  function presetTheme() { return structuredClone(state.savedThemes.find(t => t.id === state.preset)?.theme || core('preset',state.preset)); }
  function rebuildSettings() { settings({theme:state.theme,preset:presetTheme(),fonts:state.fonts,onChange:changeTheme,onReset:resetSection}); }
  function changeTheme(path,value) {
    const previous = structuredClone(state.theme);
    try {
      let target = state.theme; const parts = path.split('.');
      for (const part of parts.slice(0,-1)) target = target[part];
      target[parts.at(-1)] = value;
      compileTheme(['page.toc','page.toc_depth','page.heading_numbers','code.line_numbers','code.avoid_break_under_lines','diagrams.captions','diagrams.numbering'].includes(path)); scheduleSave();
      return true;
    } catch(error) { Object.assign(state.theme,previous); toast(error.message); return false; }
  }
  function resetSection(section) { const preset = presetTheme(); state.theme[section] = structuredClone(preset[section]); rebuildSettings(); compileTheme(true); scheduleSave(); }
  $('preset').addEventListener('change',() => { state.preset = $('preset').value; state.theme = presetTheme(); rebuildSettings(); compileTheme(true); scheduleSave(); });
  $('reset-theme').onclick = () => { state.theme = presetTheme(); rebuildSettings(); compileTheme(true); scheduleSave(); toast('Theme reset to its preset'); };
  $('filename').addEventListener('input',scheduleSave);
  $('save-theme').onclick = () => {
    const dialog = document.createElement('dialog'); dialog.className='save-dialog';
    const form = document.createElement('form'); form.method='dialog';
    const heading = document.createElement('h2'); heading.textContent='Save your theme';
    const label=document.createElement('label'); label.textContent='Theme name';
    const input=document.createElement('input'); input.value=state.theme.name === 'Default' ? 'My theme' : state.theme.name; input.required=true; label.append(input);
    const actions=document.createElement('div'); const cancel=document.createElement('button'); cancel.type='button';cancel.textContent='Cancel';cancel.className='button subtle';cancel.onclick=()=>dialog.close();
    const submit=document.createElement('button');submit.className='button primary';submit.textContent='Save theme';actions.append(cancel,submit);form.append(heading,label,actions);dialog.append(form);document.body.append(dialog);
    form.onsubmit=()=>{const id='custom:'+Date.now();state.theme.name=input.value.trim()||'My theme';state.savedThemes.push({id,name:state.theme.name,theme:structuredClone(state.theme)});localStorage.setItem(THEMES_KEY,JSON.stringify(state.savedThemes));state.preset=id;presets();scheduleSave();toast('Theme saved in this browser');};
    dialog.addEventListener('close',()=>dialog.remove());dialog.showModal();input.select();
  };
  $('export-theme').onclick=()=>{try{download(new Blob([core('export',state.theme)],{type:'text/plain'}),(state.theme.name||'theme')+'.mdpdf-theme.toml');}catch(error){toast(error.message);}};
  $('import-theme').onclick=()=>$('theme-file').click();
  $('theme-file').onchange=async()=>{const file=$('theme-file').files[0];if(!file)return;try{if(file.size>50*1024)throw new Error('Theme exceeds 50 KB');state.theme=core('import',await file.text());state.preset='default';presets();rebuildSettings();compileTheme(true);scheduleSave();toast('Theme imported');}catch(error){toast(error.message);}finally{$('theme-file').value='';}};
  const filename = extension => ($('filename').value || 'document.md').replace(/\.(md|markdown|pdf)$/i,'')+'.'+extension;
  function printDocument() {
    if(!window.mdpdf.ready){toast('Wait for the preview to finish before printing');return;}
    if($('strict-diagrams').checked && state.diagnostics.some(item=>item.kind==='Mermaid' && item.severity==='Error')){toast('Fix diagram errors before printing');return;}
    post({type:'print',revision:state.revision,title:filename('pdf').replace(/\.pdf$/i,'')});
  }
  $('export-pdf').onclick=printDocument;
  document.addEventListener('keydown',event=>{if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='p'){event.preventDefault();printDocument();}});
  $('zoom').onchange=()=>{state.zoom=$('zoom').value;resizePaper();scheduleSave();};
  const zoomStep=delta=>{const next=Math.max(.5,Math.min(2,Math.round((state.scale+delta)*100)/100));state.zoom=String(next);let option=[...$('zoom').options].find(o=>o.value===state.zoom);if(!option){option=new Option(`${Math.round(next*100)}%`,state.zoom);$('zoom').add(option);}$('zoom').value=state.zoom;resizePaper();scheduleSave();};
  $('zoom-in').onclick=()=>zoomStep(.1);$('zoom-out').onclick=()=>zoomStep(-.1);
  $('toggle-guides').onclick=()=>{state.guides=!state.guides;$('toggle-guides').classList.toggle('enabled',state.guides);$('toggle-guides').setAttribute('aria-pressed',String(state.guides));post({type:'guides',enabled:state.guides});scheduleSave();};
  $('toggle-sync').onclick=()=>{state.sync=!state.sync;$('toggle-sync').classList.toggle('enabled',state.sync);$('toggle-sync').setAttribute('aria-pressed',String(state.sync));};
  $('ui-theme').onclick=()=>{state.dark=!state.dark;document.documentElement.classList.toggle('light-ui',!state.dark);editor.dispatch({effects:editorTheme.reconfigure(state.dark?oneDark:[])});scheduleSave();};
  $('search-document').onclick=()=>{openSearchPanel(editor);editor.focus();};
  $('toggle-settings').onclick=()=>{$('workspace').classList.toggle('settings-closed');resizePaper();};
  for(const button of document.querySelectorAll('[data-pane]'))button.onclick=()=>{$('workspace').dataset.mobilePane=button.dataset.pane;for(const b of document.querySelectorAll('[data-pane]'))b.classList.toggle('active',b===button);resizePaper();};
  const menuTheme=document.createElement('button');menuTheme.id='menu-ui-theme';menuTheme.textContent='Switch interface theme';menuTheme.onclick=()=>{$('ui-theme').click();$('document-actions').classList.add('hidden');};$('document-actions').append(menuTheme);
  $('document-menu').onclick=()=>{$('document-actions').classList.toggle('hidden');};
  document.addEventListener('click',event=>{if(!event.target.closest('#document-actions,#document-menu'))$('document-actions').classList.add('hidden');});
  const replaceDocument=md=>{muteEditor=performance.now()+250;editor.dispatch({changes:{from:0,to:editor.state.doc.length,insert:md},selection:{anchor:0},effects:EditorView.scrollIntoView(0,{y:'start'})});$('preview-scroll').scrollTop=0;};
  $('new-document').onclick=()=>{try{localStorage.setItem('mdpdf:previous-draft',state.markdown);}catch{}replaceDocument('');$('filename').value='untitled.md';state.cache.clear();$('document-actions').classList.add('hidden');editor.focus();};
  $('download-md').onclick=()=>{download(new Blob([state.markdown],{type:'text/markdown'}),filename('md'));$('document-actions').classList.add('hidden');};
  $('download-html').onclick=async()=>{try{const html=await renderer.exportHtml(state.markdown,state.theme);download(new Blob([html],{type:'text/html'}),filename('html'));}catch(error){toast(error.message);}$('document-actions').classList.add('hidden');};
  let fileGeneration=0;
  async function openMarkdown(file){if(file.size>2*1024*1024){toast('Markdown exceeds 2 MB');return;}const current=++fileGeneration;const text=await file.text();if(current!==fileGeneration)return;replaceDocument(text);$('filename').value=file.name;scheduleSave();await renderPreview();}
  $('open-file').onclick=()=>$('markdown-file').click();$('markdown-file').onchange=async()=>{const file=$('markdown-file').files[0];if(file)await openMarkdown(file);$('markdown-file').value='';};
  function insert(value){const range=editor.state.selection.main;editor.dispatch({changes:{from:range.from,to:range.to,insert:value},selection:{anchor:range.from+value.length}});editor.focus();}
  $('insert-snippet').onchange=()=>{const key=$('insert-snippet').value;$('insert-snippet').value='';if(key==='image')$('image-file').click();else if(SNIPPETS[key])insert(SNIPPETS[key]);};
  async function importFile(file){const asset=await assets.import(file);if(assets.warning){toast(assets.warning);assets.warning=null;}return asset;}
  async function image(file){try{const asset=await importFile(file);if(asset.kind!=='image')throw new Error('Choose a PNG, JPEG or WebP image');insert(`![${file.name.replace(/[\[\]]/g,'')}](asset://${asset.id})`);}catch(error){toast(error.message);}}
  $('image-file').onchange=async()=>{const file=$('image-file').files[0];if(file)await image(file);$('image-file').value='';};
  async function registerFont(asset){try{const face=await new FontFace('assetfont-'+asset.id,`url(${asset.dataUrl})`).load();document.fonts.add(face);}catch{toast(`Could not load font: ${asset.name}`);}}
  $('upload-font').onclick=()=>$('font-file').click();
  $('font-file').onchange=async()=>{const file=$('font-file').files[0];if(!file)return;try{const asset=await importFile(file);if(asset.kind!=='font')throw new Error('Choose a TTF, OTF or WOFF2 font');await registerFont(asset);if(!state.fonts.some(font=>font.id===asset.id))state.fonts.push(asset);rebuildSettings();changeTheme('typography.body_font','assetfont-'+asset.id);rebuildSettings();toast('Font imported and applied to body text');}catch(error){toast(error.message);}finally{$('font-file').value='';}};
  document.addEventListener('dragover',event=>{if(event.dataTransfer?.types.includes('Files'))event.preventDefault();});
  document.addEventListener('drop',async event=>{
    const files=[...(event.dataTransfer?.files||[])];if(!files.length)return;
    event.preventDefault();event.stopPropagation();
    try{
      const documents=files.filter(file=>/\.(md|markdown|txt)$/i.test(file.name));
      if(documents.length){
        for(const file of files.filter(file=>!documents.includes(file))){const asset=await importFile(file);if(asset.kind==='font'){await registerFont(asset);if(!state.fonts.some(font=>font.id===asset.id))state.fonts.push(asset);}}
        rebuildSettings();await openMarkdown(documents.at(-1));
      }else for(const file of files)await image(file);
    }catch(error){toast(error.message);}
  },{capture:true});
  $('editor').addEventListener('paste',event=>{const files=[...(event.clipboardData?.files||[])];if(files.length){event.preventDefault();for(const file of files)image(file);}});
  function resizer(id,variable,min,max,invert){const node=$(id);node.onpointerdown=event=>{node.setPointerCapture(event.pointerId);const origin=event.clientX;const initial=parseFloat(getComputedStyle($('workspace')).getPropertyValue(variable))||min;const move=event=>{$('workspace').style.setProperty(variable,Math.max(min,Math.min(max,initial+(event.clientX-origin)*(invert?-1:1)))+'px');resizePaper();};node.onpointermove=move;node.onpointerup=()=>{node.onpointermove=null;};};node.onkeydown=event=>{if(event.key==='ArrowLeft'||event.key==='ArrowRight'){event.preventDefault();const current=parseFloat(getComputedStyle($('workspace')).getPropertyValue(variable))||min;$('workspace').style.setProperty(variable,Math.max(min,Math.min(max,current+(event.key==='ArrowLeft'?-20:20)*(invert?-1:1)))+'px');resizePaper();}};}
  resizer('editor-resizer','--editor-width',260,650,false);resizer('settings-resizer','--settings-width',245,440,true);
  window.addEventListener('beforeunload',save);
  await Promise.all(state.fonts.map(registerFont));
  if(assets.warning)toast(assets.warning);
  presets();rebuildSettings();stats();resizePaper();save();await renderPreview();
}
window.addEventListener('mdpdf-ready',()=>mount().catch(error=>{console.error(error);toast(error.message);}));
if(window.mdpdfCore)mount().catch(error=>{console.error(error);toast(error.message);});
