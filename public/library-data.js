export function draftContent(doc){return JSON.stringify([doc.markdown,doc.theme,doc.preset||'default',doc.filename]);}
export function normalizeTags(value){return [...new Set(String(value).split(',').map(tag=>tag.trim().toLocaleLowerCase()).filter(Boolean))].slice(0,20);}
export function searchDocuments(docs,{query='',folder='',tag='',pinned=false}={}){
  const terms=query.toLocaleLowerCase().trim().split(/\s+/).filter(Boolean);
  return docs.filter(doc=>(!folder||(folder==='__unfiled__'?!doc.folder:doc.folder===folder))&&(!tag||(doc.tags||[]).includes(tag))&&(!pinned||doc.pinned)&&terms.every(term=>[doc.filename,doc.markdown,doc.folder,...(doc.tags||[])].join('\n').toLocaleLowerCase().includes(term))).sort((a,b)=>Number(!!b.pinned)-Number(!!a.pinned)||b.updated-a.updated);
}
export function assetUsage(assets,documents,current){return new Map(assets.map(asset=>[asset.id,documents.filter(doc=>JSON.stringify([doc.markdown,doc.theme,doc.history]).includes(asset.id)).map(doc=>({id:doc.id,name:doc.filename})).concat(current&&JSON.stringify([current.markdown,current.theme]).includes(asset.id)&&!documents.some(doc=>doc.id===current.id&&JSON.stringify([doc.markdown,doc.theme,doc.history]).includes(asset.id))?[{id:current.id,name:current.filename+' (current draft)'}]:[])]));}
export class DocumentConflict extends Error{constructor(latest){super('This document changed in another tab. Review the changes before saving.');this.name='DocumentConflict';this.latest=latest;}}
// Read and compare inside one transaction so simultaneous tabs cannot both win a save.
export function commitDocument(db,{id,draft,base,baseline,force=false,metadata}){
  return new Promise((resolve,reject)=>{
    const tx=db.transaction('documents','readwrite'),store=tx.objectStore('documents'),request=store.get(id);let result,error;
    request.onsuccess=()=>{
      const previous=request.result,revision=previous?.revision||0,content=draftContent(draft),old=previous&&draftContent(previous);
      if(previous&&revision!==base&&old!==content&&old!==baseline){
        if(content===baseline){result={stale:true,document:previous};return;}
        error=new DocumentConflict(previous);return;
      }
      if(previous&&Number.isInteger(previous.revision)&&old===content&&(!metadata||Object.entries(metadata).every(([key,value])=>JSON.stringify(previous[key])===JSON.stringify(value)))){result={unchanged:true,document:previous};return;}
      const now=Date.now(),history=previous?.history||[];
      if(previous&&previous.markdown!==draft.markdown&&(force||!history.length||now-history[0].at>30000))history.unshift({at:previous.updated,markdown:previous.markdown,theme:previous.theme,preset:previous.preset,filename:previous.filename});
      const document={...previous,markdown:draft.markdown,theme:draft.theme,preset:draft.preset||'default',filename:draft.filename,...metadata,id,revision:revision+1,updated:now,history:history.slice(0,30)};
      store.put(document);result={document};
    };
    tx.oncomplete=()=>error?reject(error):resolve(result);tx.onerror=()=>reject(tx.error);tx.onabort=()=>reject(tx.error);
  });
}
