import {assetManager} from './asset-manager.js';
import {commitDocument,draftContent,normalizeTags,searchDocuments} from './library-data.js';
import {documentFeatures} from './document-features.js';
import {StateEffect} from './vendor/editor.js';
import {headings,parseTable,tableMarkdown,tsvTable} from './markdown-tools.js';
const el=(tag,text,props={})=>Object.assign(document.createElement(tag),{textContent:text,...props});
const button=(text,action)=>{const b=el('button',text,{type:'button'});b.onclick=action;return b;};
function database() {
  return new Promise((resolve,reject)=>{const request=indexedDB.open('mdpdf-library',1);request.onupgradeneeded=()=>{for(const name of ['documents','assets'])request.result.createObjectStore(name,{keyPath:'id'});};request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);});
}
export async function workspaceTools(api) {
  const {editor,EditorView,EditorState,toast}=api;
  let features;
  let db;try{db=await database();}catch{toast('Document library is unavailable. You can still edit and download Markdown.');}
  const store=(name,mode,action)=>new Promise((resolve,reject)=>{if(!db){reject(new Error('Local library unavailable'));return;}const tx=db.transaction(name,mode);const req=action(tx.objectStore(name));tx.oncomplete=()=>resolve(req.result);tx.onerror=()=>reject(tx.error);tx.onabort=()=>reject(tx.error);});
  const get=(name,id)=>store(name,'readonly',s=>s.get(id));
  const put=(name,value)=>store(name,'readwrite',s=>s.put(value));
  const bases=new Map();let active;
  try{active=sessionStorage.getItem('mdpdf:active-document')||localStorage.getItem('mdpdf:active-document');}catch{}
  active ||= crypto.randomUUID();
  const setActive=id=>{if(active!==id)try{sessionStorage.removeItem('mdpdf:draft:v1');}catch{}active=id;try{sessionStorage.setItem('mdpdf:active-document',id);localStorage.setItem('mdpdf:active-document',id);}catch{}};
  setActive(active);
  function baseline(doc){const value={revision:doc?.revision||0,content:doc?draftContent(doc):null};bases.set(active,value);try{sessionStorage.setItem('mdpdf:document-base',JSON.stringify({id:active,...value}));}catch{}}
  const banner=el('aside','',{className:'conflict-banner',hidden:true});banner.setAttribute('role','status');banner.setAttribute('aria-live','polite');banner.append(el('span','This document changed in another tab. Your edits are kept here.'),button('Review changes',()=>reviewConflict()));document.body.append(banner);
  let remote,recoverySaving=Promise.resolve();
  function recoveryKey(id){return 'mdpdf:conflict-copy:'+id;}
  function ensureRecovery(draft,id=active){
    const snapshot=structuredClone(draft);let recoveryId;try{recoveryId=sessionStorage.getItem(recoveryKey(id));}catch{}recoveryId ||= crypto.randomUUID();try{sessionStorage.setItem(recoveryKey(id),recoveryId);}catch{}
    recoverySaving=recoverySaving.catch(()=>{}).then(async()=>{const previous=await get('documents',recoveryId),copy={...snapshot,filename:snapshot.filename.replace(/(\.md|\.markdown)?$/i,' (recovery)$1')};const result=await commitDocument(db,{id:recoveryId,draft:copy,base:previous?.revision||0,baseline:previous?draftContent(previous):null,metadata:{recoveryOf:id},force:true});notify(result.document);return result.document;});return recoverySaving;
  }
  function changed(doc){remote=doc;const current=api.draft();if(db&&draftContent(current)!==bases.get(active)?.content&&draftContent(current)!==draftContent(doc))ensureRecovery(current).catch(error=>toast('Could not keep a recovery copy: '+error.message));banner.hidden=false;document.getElementById('save-state').textContent='Changed in another tab';}
  function clearConflict(){if(remote)try{sessionStorage.removeItem(recoveryKey(remote.id));}catch{}remote=null;banner.hidden=true;}
  const channel=typeof BroadcastChannel==='function'?new BroadcastChannel('mdpdf-library'):null;
  function notify(doc){channel?.postMessage({id:doc.id,revision:doc.revision});try{localStorage.setItem('mdpdf:library-update',JSON.stringify({id:doc.id,revision:doc.revision,nonce:crypto.randomUUID()}));}catch{}}
  async function receive(message){if(!db||message?.id!==active||message.revision===bases.get(active)?.revision)return;const doc=await get('documents',active);if(doc&&doc.revision!==bases.get(active)?.revision){if(draftContent(doc)===bases.get(active)?.content)baseline(doc);else if(draftContent(doc)===draftContent(api.draft())){baseline(doc);clearConflict();}else changed(doc);}}
  if(channel)channel.onmessage=event=>{receive(event.data).catch(()=>{});};
  window.addEventListener('storage',event=>{if(event.key==='mdpdf:library-update')try{receive(JSON.parse(event.newValue)).catch(()=>{});}catch{}});
  let saving=Promise.resolve();
  const save=(force=false)=>{
    api.persistDraft?.();if(!db)return Promise.resolve({localOnly:true});
    const draft=structuredClone(api.draft()),id=active;
    saving=saving.catch(()=>{}).then(async()=>{
      const base=bases.get(id)||{revision:0,content:null};
      try{
        const result=await commitDocument(db,{id,draft,base:base.revision,baseline:base.content,force});
        if(id===active){if(result.stale)changed(result.document);else {baseline(result.document);clearConflict();}}
        else if(!result.stale)bases.set(id,{revision:result.document.revision,content:draftContent(result.document)});
        if(!result.stale&&!result.unchanged)notify(result.document);return result;
      }catch(error){if(error.name==='DocumentConflict'&&id===active)changed(error.latest);else toast('Could not save the local document: '+error.message);throw error;}
    });return saving;
  };
  const safe=action=>async()=>{try{await action();}catch(error){if(error.name!=='DocumentConflict')toast(error.message);}};
  async function adopt(doc){await restoreAssets(doc);setActive(doc.id);baseline(doc);clearConflict();await api.apply(doc);api.persistDraft?.();}
  async function reviewConflict(){if(document.getElementById('document-conflict'))return;const id=active,latest=await get('documents',id);if(!latest)return;const local=structuredClone(api.draft()),d=dialog('Review document changes','document-conflict');d.append(el('p','The saved version and your current edits are shown below. Loading the saved version keeps changed local text as a recovery copy.'));
    const panes=el('div','',{className:'comparison-panes'});for(const [title,version] of [['Your current draft',local],['Saved in another tab',latest]]){const section=el('section');section.append(el('h3',title),el('p',version.filename+' · '+(version.theme?.name||version.preset)),el('pre',version.markdown));if(JSON.stringify(local.theme)!==JSON.stringify(latest.theme)){const details=el('details');details.append(el('summary','Theme settings'),el('pre',JSON.stringify(version.theme,null,2)));section.append(details);}panes.append(section);}d.append(panes);
    d.append(button('Load saved version',safe(async()=>{if(active!==id)return;const current=api.draft(),base=bases.get(id);if(draftContent(current)!==base?.content&&draftContent(current)!==draftContent(latest))await ensureRecovery(current,id);const fresh=await get('documents',id);d.close();await adopt(fresh);})),button('Keep my draft as a new document',safe(async()=>{const copy=await ensureRecovery(api.draft(),id);d.close();await adopt(copy);})));
  }
  async function restoreAssets(draft) {
    if(!db)return;
    const refs=JSON.stringify([draft.markdown,draft.theme]);
    const assets=await store('assets','readonly',s=>s.getAll());
    for(const asset of assets)if(refs.includes(asset.id))await api.restoreAsset(asset);
  }
  async function open(draft) {await save();await adopt(await get('documents',draft.id));await save();}
  async function create() {await save();setActive(crypto.randomUUID());baseline(null);clearConflict();await api.apply({...api.draft(),markdown:'',filename:'untitled.md'});await save();document.getElementById('document-actions').classList.add('hidden');editor.focus();}
  function dialog(title,id) {
    const opener=document.activeElement,d=el('dialog','',{className:'tools-dialog',id});d.setAttribute('aria-label',title);const heading=el('h2',title,{id:id+'-title'});d.setAttribute('aria-labelledby',heading.id);
    const header=el('header');header.append(heading,button('Close',()=>d.close()));d.append(header);
    d.addEventListener('close',()=>{d.remove();if(editor.hasFocus)return;if(opener?.isConnected&&!opener.closest('dialog:not([open])'))opener.focus({preventScroll:true});else if(!document.querySelector('dialog[open]'))editor.focus();});document.body.append(d);d.showModal();return d;
  }
  async function library() {
    await save(true);const d=dialog('Local documents','document-library');d.append(el('p','Saved in this browser. Download important documents for backup.'));
    const name=el('input','',{value:api.draft().filename});name.setAttribute('aria-label','Document name');
    d.append(name,button('Rename',safe(async()=>{await api.rename(name.value.trim()||'untitled.md');await save();d.close();await library();})),button('New document',safe(async()=>{d.close();await create();})));
    let docs=await store('documents','readonly',s=>s.getAll());
    const query=el('input','',{placeholder:'Search titles and content'}),folder=el('select'),tag=el('select'),pinned=el('input','',{type:'checkbox'}),pinLabel=el('label','Pinned only'),count=el('p',''),list=el('div');
    query.setAttribute('aria-label','Search documents');folder.setAttribute('aria-label','Filter by folder');tag.setAttribute('aria-label','Filter by tag');pinLabel.prepend(pinned);count.setAttribute('role','status');count.setAttribute('aria-live','polite');
    function filters(){const current=folder.value,selectedTag=tag.value;folder.replaceChildren(el('option','All folders',{value:''}),el('option','Unfiled',{value:'__unfiled__'}));for(const value of [...new Set(docs.map(doc=>doc.folder).filter(Boolean))].sort())folder.append(el('option',value,{value}));tag.replaceChildren(el('option','All tags',{value:''}));for(const value of [...new Set(docs.flatMap(doc=>doc.tags||[]))].sort())tag.append(el('option',value,{value}));folder.value=current;tag.value=selectedTag;}
    async function organize(doc,metadata){let result;try{result=await commitDocument(db,{id:doc.id,draft:doc,base:doc.revision||0,baseline:draftContent(doc),metadata,force:true});}catch(error){if(doc.id===active&&error.name==='DocumentConflict')changed(error.latest);else throw new Error('Document changed. Reopen the library before organizing it.');throw error;}if(result.stale)throw new Error('Document changed. Reopen the library.');if(doc.id===active){const base=bases.get(active);if(base?.content===draftContent(doc))baseline(result.document);}notify(result.document);docs=await store('documents','readonly',s=>s.getAll());filters();render();const action=Object.hasOwn(metadata,'pinned')?'pin':'organize';[...list.querySelectorAll('[data-document]')].find(node=>node.dataset.document===doc.id&&node.dataset.action===action)?.focus();}
    function render(){list.replaceChildren();const matches=searchDocuments(docs,{query:query.value,folder:folder.value,tag:tag.value,pinned:pinned.checked});count.textContent=`${matches.length} document${matches.length===1?'':'s'}`;if(!matches.length)list.append(el('p','No matching documents. Try another search or clear the filters.'));
      for(const doc of matches){const row=el('div','',{className:'library-row'}),pin=button(doc.pinned?'Unpin':'Pin',safe(async()=>{await organize(doc,{pinned:!doc.pinned});}));pin.dataset.document=doc.id;pin.dataset.action='pin';pin.setAttribute('aria-label',(doc.pinned?'Unpin ':'Pin ')+doc.filename);pin.setAttribute('aria-pressed',String(!!doc.pinned));
        row.append(button((doc.id===active?'● ':'')+doc.filename,safe(async()=>{d.close();await open(doc);})),el('small',new Date(doc.updated).toLocaleString()),button('History',()=>history(doc)),pin);
        const details=el('details'),summary=el('summary','Organize '+doc.filename),folderInput=el('input','',{value:doc.folder||'',placeholder:'Folder'}),tags=el('input','',{value:(doc.tags||[]).join(', '),placeholder:'Tags separated by commas'});folderInput.setAttribute('aria-label','Folder for '+doc.filename);tags.setAttribute('aria-label','Tags for '+doc.filename);summary.dataset.document=doc.id;summary.dataset.action='organize';const folderLabel=el('label','Folder'),tagsLabel=el('label','Tags');folderLabel.append(folderInput);tagsLabel.append(tags);details.append(summary,folderLabel,tagsLabel,button('Save organization',safe(async()=>{await organize(doc,{folder:folderInput.value.trim().slice(0,80),tags:normalizeTags(tags.value)});count.textContent='Organization saved';})));row.append(details);list.append(row);
      }
    }
    d.append(query,folder,tag,pinLabel,count,list);filters();query.oninput=render;folder.onchange=render;tag.onchange=render;pinned.onchange=render;render();query.focus();
  }

  function history(doc) {
    const d=dialog('Document history','document-history');
    if(!doc.history.length)d.append(el('p','Earlier versions appear as you write. Up to 30 restore points are kept.'));
    for(const version of doc.history) {
      const details=el('details');details.append(el('summary',new Date(version.at).toLocaleString()),el('pre',version.markdown.slice(0,12000)));
      details.append(button('Restore this version',safe(async()=>{await save();if(doc.id!==active)await open(doc);await restoreAssets(version);await api.apply(version);await save(true);d.close();document.getElementById('document-library')?.close();})));d.append(details);
    }
  }
  function replace(from,to,text,selection) {editor.dispatch({changes:{from,to,insert:text},selection:selection||{anchor:from+text.length},userEvent:'input'});editor.focus();}
  function wrap(mark,close=mark,placeholder='text') {
    const {from,to}=editor.state.selection.main, selected=editor.state.sliceDoc(from,to)||placeholder;
    if(from>=mark.length&&editor.state.sliceDoc(from-mark.length,from)===mark&&editor.state.sliceDoc(to,to+close.length)===close)replace(from-mark.length,to+close.length,selected,{anchor:from-mark.length,head:from-mark.length+selected.length});
    else replace(from,to,mark+selected+close,{anchor:from+mark.length,head:from+mark.length+selected.length});
  }
  function prefix(mark) {
    const range=editor.state.selection.main, first=editor.state.doc.lineAt(range.from),last=editor.state.doc.lineAt(range.to>range.from?range.to-1:range.to);
    const lines=editor.state.sliceDoc(first.from,last.to).split('\n');const remove=lines.every(line=>line.startsWith(mark));
    replace(first.from,last.to,lines.map(line=>remove?line.slice(mark.length):mark+(/^#{1,6} $/.test(mark)?line.replace(/^ {0,3}#{1,6}\s+/,''):line)).join('\n'));
  }
  function link() {const r=editor.state.selection.main,text=editor.state.sliceDoc(r.from,r.to)||'link text';replace(r.from,r.to,`[${text}](https://)`,{anchor:r.from+text.length+3,head:r.from+text.length+11});}
  function currentTable() {
    const doc=editor.state.doc, cursor=doc.lineAt(editor.state.selection.main.head);let first=cursor.number,last=first;
    if(!cursor.text.includes('|'))return null;
    while(first>1&&doc.line(first-1).text.includes('|'))first--;
    while(last<doc.lines&&doc.line(last+1).text.includes('|'))last++;
    const from=doc.line(first).from,to=doc.line(last).to,parsed=parseTable(doc.sliceString(from,to));return parsed?{from,to,...parsed}:null;
  }
  function table() {
    const existing=currentTable(),selection=editor.state.selection.main;let rows=existing?.rows||[['Column 1','Column 2'],['','']],align=existing?.align||[];
    const d=dialog(existing?'Edit table':'Insert table','table-tools'),grid=el('div','',{className:'table-grid'});
    function render(){grid.replaceChildren();const t=el('table'),head=el('tr');t.append(el('caption','First row contains column headings.'));
      for(let c=0;c<rows[0].length;c++){const th=el('th'),select=el('select');th.scope='col';select.setAttribute('aria-label',`Column ${c+1} alignment`);for(const value of ['','left','center','right'])select.append(el('option',value||'Default',{value}));select.value=align[c]||'';select.onchange=()=>align[c]=select.value;th.append(select,button('Remove column '+(c+1),()=>{if(rows[0].length>1){rows.forEach(row=>row.splice(c,1));align.splice(c,1);render();grid.querySelector('input')?.focus();}}));head.append(th);}t.append(head);
      rows.forEach((row,r)=>{const tr=el('tr');for(let c=0;c<rows[0].length;c++){const td=el('td'),input=el('input','',{value:row[c]||''});input.setAttribute('aria-label',`Row ${r+1} column ${c+1}`);input.oninput=()=>row[c]=input.value;td.append(input);tr.append(td);}if(r>0){const td=el('td');td.append(button('Remove row '+(r+1),()=>{rows.splice(r,1);render();grid.querySelector('input')?.focus();}));tr.append(td);}t.append(tr);});grid.append(t);}
    d.append(grid,button('Add row',()=>{rows.push(Array(rows[0].length).fill(''));render();grid.querySelectorAll('input')[((rows.length-1)*rows[0].length)]?.focus();}),button('Add column',()=>{rows.forEach(row=>row.push(''));render();grid.querySelectorAll('input')[rows[0].length-1]?.focus();}),button('Apply table',()=>{replace(existing?.from??selection.from,existing?.to??selection.to,'\n'+tableMarkdown(rows,align)+'\n');d.close();}));render();
  }
  const outline=el('dialog','',{id:'document-outline',className:'tools-dialog outline-dialog'});outline.setAttribute('aria-label','Document outline');document.body.append(outline);
  let outlineTimer;
  function updateOutline(){if(!outline.open)return;outline.replaceChildren();const header=el('header');header.append(el('h2','Outline'),button('Close',()=>outline.close()));outline.append(header);const items=headings(editor.state.doc.toString()),line=editor.state.doc.lineAt(editor.state.selection.main.head).number,current=items.filter(h=>h.line<=line).at(-1);
    if(!items.length)outline.append(el('p','Add Markdown headings to navigate your document.'));
    let index=0;
    function branch(parentLevel){const container=el('div','',{className:'outline-items'});
      while(index<items.length&&items[index].level>parentLevel){const h=items[index++],b=button(h.text,()=>{const from=Math.min(h.from,editor.state.doc.length);editor.dispatch({selection:{anchor:from},effects:EditorView.scrollIntoView(from,{y:'start'})});api.jump(h.line);outline.close();editor.focus();});if(h===current)b.setAttribute('aria-current','location');
        if(index<items.length&&items[index].level>h.level){const children=branch(h.level),row=el('div'),toggle=button('−',()=>{children.hidden=!children.hidden;toggle.textContent=children.hidden?'+':'−';toggle.setAttribute('aria-expanded',String(!children.hidden));toggle.setAttribute('aria-label',(children.hidden?'Expand ':'Collapse ')+h.text);});children.id='outline-group-'+crypto.randomUUID();toggle.setAttribute('aria-label','Collapse '+h.text);toggle.setAttribute('aria-expanded','true');toggle.setAttribute('aria-controls',children.id);row.append(toggle,b);container.append(row,children);}else container.append(b);
      }return container;
    }
    outline.append(branch(0));}

  function showOutline(){outline.showModal();updateOutline();}
  function observeEditor(){features?.observe();editor.dispatch({effects:StateEffect.appendConfig.of(EditorView.updateListener.of(update=>{if(update.docChanged||update.selectionSet){clearTimeout(outlineTimer);outlineTimer=setTimeout(updateOutline,100);}}))});}
  const commands=[
    ['Bold','Mod+B',()=>wrap('**')],['Italic','Mod+I',()=>wrap('*')],['Inline code','',()=>wrap('`')],['Link','Mod+K',link],['Heading 1','',()=>prefix('# ')],['Heading 2','',()=>prefix('## ')],['Bullet list','',()=>prefix('- ')],['Numbered list','',()=>prefix('1. ')],['Task list','',()=>prefix('- [ ] ')],['Quote','',()=>prefix('> ')],['Code block','',()=>wrap('```\n','\n```','code')],['Insert or edit table','',table],['Format table','',()=>{const t=currentTable();if(t)replace(t.from,t.to,tableMarkdown(t.rows,t.align));else table();}],['Document outline','',showOutline],['Local documents','',safe(library)],['New document','',safe(create)],['Zen: Read','Alt+1',()=>api.zen('read')],['Zen: Write','Alt+2',()=>api.zen('write')],['Export PDF','',()=>document.getElementById('export-pdf').click()],['Change interface theme','',()=>document.getElementById('ui-theme').click()],['Find in document','Mod+F',()=>document.getElementById('search-document').click()]
  ];
  function palette(){const d=dialog('Commands','command-palette'),input=el('input','',{placeholder:'Search commands…'}),list=el('div','',{className:'command-results'});input.setAttribute('aria-label','Search commands');input.setAttribute('role','combobox');input.setAttribute('aria-expanded','true');input.setAttribute('aria-autocomplete','list');input.setAttribute('aria-controls','command-options');list.id='command-options';list.setAttribute('role','listbox');list.setAttribute('aria-label','Commands');const hint=el('p','Arrow keys choose a command. Enter runs it. Escape closes this dialog.',{id:'command-hint'});input.setAttribute('aria-describedby',hint.id);let selected=0,filtered=[];
    function render(){filtered=commands.filter(c=>c[0].toLowerCase().includes(input.value.toLowerCase()));selected=Math.min(selected,Math.max(0,filtered.length-1));list.replaceChildren();filtered.forEach((c,i)=>{const b=button(c[0],()=>run(c));b.append(el('kbd',c[1].replace('Mod+',/Mac|iPhone|iPad/.test(navigator.platform)?'⌘':'Ctrl+')));b.classList.toggle('selected',i===selected);b.setAttribute('role','option');b.setAttribute('aria-selected',String(i===selected));b.id='command-option-'+i;b.tabIndex=-1;list.append(b);});if(filtered.length)input.setAttribute('aria-activedescendant','command-option-'+selected);else{input.removeAttribute('aria-activedescendant');const empty=el('p','No matching commands.');empty.setAttribute('role','status');list.append(empty);}}
    function run(c){d.close();c[2]();}
    input.oninput=()=>{selected=0;render();};input.onkeydown=e=>{if(e.isComposing)return;if(e.key==='ArrowDown'||e.key==='ArrowUp'){e.preventDefault();selected=(selected+(e.key==='ArrowDown'?1:-1)+filtered.length)%Math.max(1,filtered.length);render();list.children[selected]?.scrollIntoView({block:'nearest'});}if(e.key==='Enter'&&filtered[selected]){e.preventDefault();run(filtered[selected]);}};d.append(input,hint,list);render();input.focus();}
  const toolbar=document.querySelector('.workspace-breadcrumb');toolbar.replaceChildren(button('Commands',palette),button('Outline',showOutline),button('Documents',safe(library)));toolbar.firstChild.title='Commands (Ctrl/Cmd+Shift+P)';toolbar.firstChild.setAttribute('aria-keyshortcuts','Control+Shift+P Meta+Shift+P');
  document.addEventListener('keydown',event=>{if(event.isComposing||event.repeat)return;const mod=(/Mac|iPhone|iPad/.test(navigator.platform)?event.metaKey:event.ctrlKey);if(!mod||event.altKey)return;
    if(event.shiftKey&&event.code==='KeyP'){event.preventDefault();event.stopPropagation();if(!document.getElementById('command-palette'))palette();return;}
    if(document.querySelector('dialog[open]')||!editor.hasFocus||event.shiftKey)return;
    const command={KeyB:0,KeyI:1,KeyK:3}[event.code];if(command!==undefined){event.preventDefault();event.stopPropagation();commands[command][2]();}
  },true);
  document.getElementById('editor').addEventListener('paste',event=>{
    if(event.defaultPrevented||event.clipboardData?.files.length)return;
    const text=event.clipboardData?.getData('text/plain')||'',r=editor.state.selection.main;
    if(/^https?:\/\/\S+$/.test(text.trim())&&r.from!==r.to){event.preventDefault();replace(r.from,r.to,`[${editor.state.sliceDoc(r.from,r.to)}](${text.trim()})`);return;}
    const table=tsvTable(text);if(table){event.preventDefault();replace(r.from,r.to,'\n'+table+'\n');return;}features?.onPaste(event);
  },true);
  if(db)try {
    const response=await fetch('/api/assets');
    if(response.ok)for(const asset of await response.json())if(!await get('assets',asset.id)){
      const response=await fetch(asset.url);if(response.ok)await put('assets',{...asset,blob:await response.blob()});
    }
  }catch(error){toast('Could not cache uploaded assets: '+error.message);}
  const existing=db?await get('documents',active):null;
  let recovery,storedBase;try{recovery=JSON.parse(sessionStorage.getItem('mdpdf:draft:v1')||'null');storedBase=JSON.parse(sessionStorage.getItem('mdpdf:document-base')||'null');}catch{}
  if(existing){
    if(recovery&&storedBase?.id===active&&draftContent(api.draft())!==storedBase.content){bases.set(active,{revision:storedBase.revision,content:storedBase.content});if((existing.revision||0)!==storedBase.revision&&draftContent(existing)!==storedBase.content)changed(existing);await restoreAssets(api.draft());}
    else if(recovery&&!storedBase&&draftContent(api.draft())!==draftContent(existing)){bases.set(active,{revision:-1,content:null});changed(existing);await restoreAssets(api.draft());}
    else await adopt(existing);
  }else {baseline(null);await restoreAssets(api.draft());}
  observeEditor();
  const showAssets=assetManager({...api,el,button,dialog,safe,store,put,save,replace,activeId:()=>active});
  commands.push(['Asset manager','',safe(showAssets)]);
  document.getElementById('document-actions').append(button('Asset manager',safe(showAssets)));
  features=documentFeatures({...api,dialog,el,button,safe,commands,replace,store,get,put,save,restoreAssets,
    createDraft:async draft=>{await save(true);setActive(crypto.randomUUID());baseline(null);clearConflict();await api.apply(draft);await save();}
  });
  const oldNew=document.getElementById('new-document');oldNew.onclick=safe(create);
  return {save,create,table,observeEditor,preflight:()=>features.preflight(),onMessage:message=>features.onMessage(message),beforeImport:async()=>{await save();setActive(crypto.randomUUID());baseline(null);clearConflict();},keepAsset:async(asset,file)=>{if(db){const old=await get('assets',asset.id);await put('assets',{...old,...asset,blob:file});}},palette};
}
