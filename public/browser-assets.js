// Files stay in this browser. No upload API or application server is involved.
const MAX_ASSETS = 100, MAX_BYTES = 64 * 1024 * 1024;
export const assetUrl = path => new URL(path.replace(/^\//,''), new URL('./',import.meta.url));
export const abortError = () => new DOMException('Superseded preview','AbortError');
export function nonce() {
  return btoa(String.fromCharCode(...crypto.getRandomValues(new Uint8Array(24))));
}
function asDataUrl(bytes,mime) {
  return new Promise((resolve,reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(new Blob([bytes],{type:mime}));
  });
}
function imageMime(bytes) {
  if (bytes[0]===137 && bytes[1]===80 && bytes[2]===78 && bytes[3]===71) return 'image/png';
  if (bytes[0]===255 && bytes[1]===216 && bytes[2]===255) return 'image/jpeg';
  if (String.fromCharCode(...bytes.slice(0,4))==='RIFF' && String.fromCharCode(...bytes.slice(8,12))==='WEBP') return 'image/webp';
  return null;
}
export async function readRemoteImage(url,signal) {
  const response = await fetch(url,{credentials:'omit',referrerPolicy:'no-referrer',signal});
  if (!response.ok || !response.body) throw new Error('Image cannot be read by this browser');
  if (Number(response.headers.get('content-length'))>10*1024*1024) throw new Error('Image exceeds 10 MB');
  const reader=response.body.getReader(),chunks=[]; let size=0;
  try {
    while (true) {
      const {done,value}=await reader.read(); if (done) break;
      size+=value.length; if (size>10*1024*1024) throw new Error('Image exceeds 10 MB');
      chunks.push(value);
    }
  } finally { await reader.cancel().catch(()=>{}); }
  const bytes=new Uint8Array(size); let offset=0;
  for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.length;}
  const mime=imageMime(bytes); if(!mime)throw new Error('Choose a PNG, JPEG or WebP image');
  return {dataUrl:await asDataUrl(bytes,mime),size};
}
export class LocalAssets {
  assets=new Map(); warning=null; db=null;
  async open() {
    try {
      this.db=await new Promise((resolve,reject)=>{
        const request=indexedDB.open('mdpdf:assets:v1',1);
        request.onupgradeneeded=()=>request.result.createObjectStore('files',{keyPath:'id'});
        request.onsuccess=()=>resolve(request.result); request.onerror=()=>reject(request.error);
        request.onblocked=()=>reject(new Error('Storage is blocked'));
      });
      const files=await new Promise((resolve,reject)=>{
        const request=this.db.transaction('files').objectStore('files').getAll();
        request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);
      });
      for(const asset of files)this.assets.set(asset.id,asset);
    } catch { this.warning='Local file storage is unavailable. Imported files will last until this tab closes.'; }
    return [...this.assets.values()];
  }
  async import(file) {
    const font=/\.(ttf|otf|woff2)$/i.exec(file.name)?.[1].toLowerCase();
    const cap=(font?5:10)*1024*1024;
    if(file.size>cap)throw new Error(`File exceeds ${font?5:10} MB`);
    const bytes=await file.arrayBuffer();
    const mime=font?'font/'+font:imageMime(new Uint8Array(bytes));
    if(!mime)throw new Error('Choose a PNG, JPEG, WebP, TTF, OTF or WOFF2 file');
    if(font)await new FontFace('mdpdf-local-check',bytes).load();
    const id=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(byte=>byte.toString(16).padStart(2,'0')).join('');
    const previous=this.assets.get(id);
    if(!previous && this.assets.size>=MAX_ASSETS)throw new Error('This browser already contains 100 imported files');
    if(font && !previous && [...this.assets.values()].filter(asset=>asset.kind==='font').length>=10)throw new Error('This browser already contains 10 custom fonts');
    const total=[...this.assets.values()].reduce((sum,asset)=>sum+asset.size,0);
    if(total+(previous?0:file.size)>MAX_BYTES)throw new Error('Local assets exceed 64 MB');
    const aliases=[...new Set([...(previous?.aliases||[]),file.name,file.webkitRelativePath].filter(Boolean))];
    const dataUrl=previous?.dataUrl || await asDataUrl(bytes,mime);
    const asset={id,name:file.name,aliases,kind:font?'font':'image',mime,size:file.size,dataUrl,url:dataUrl};
    this.assets.set(id,asset);
    if(this.db)try{
      await new Promise((resolve,reject)=>{
        const transaction=this.db.transaction('files','readwrite');
        transaction.objectStore('files').put(asset);
        transaction.oncomplete=resolve;transaction.onerror=()=>reject(transaction.error);transaction.onabort=()=>reject(transaction.error);
      });
    }catch{this.warning='Browser storage is full. This file is available for this session but might not survive a reload.';}
    return asset;
  }
  imageMap(markdown) {
    const images={};
    for(const asset of this.assets.values()){
      if(asset.kind!=='image')continue;
      const names=asset.aliases||[asset.name];
      if(!markdown.includes(asset.id) && !names.some(name=>markdown.includes(name)||markdown.includes(encodeURI(name))))continue;
      images['asset://'+asset.id]=asset.dataUrl;
      for(const name of names){images[name]=asset.dataUrl;images[encodeURI(name)]=asset.dataUrl;}
    }
    return images;
  }
}
const resources=new Map();
export function resource(path,mime) {
  if(!resources.has(path))resources.set(path,(async()=>{
    const response=await fetch(assetUrl(path));
    if(!response.ok)throw new Error('Could not load bundled resource: '+path);
    return asDataUrl(await response.arrayBuffer(),mime);
  })().catch(error=>{resources.delete(path);throw error;}));
  return resources.get(path);
}
export async function embedCss(css,characters) {
  const points=[...new Set([...characters].map(character=>character.codePointAt(0)))].sort((a,b)=>a-b);
  const hasPoint=(low,high)=>{
    let left=0,right=points.length;
    while(left<right){const middle=(left+right)>>>1;if(points[middle]<low)left=middle+1;else right=middle;}
    return left<points.length&&points[left]<=high;
  };
  const faces=[...css.matchAll(/@font-face\s*\{[^}]*\}/g)];
  const replacements=await Promise.all(faces.map(async match=>{
    const ranges=match[0].match(/unicode-range:\s*([^;}]+)/)?.[1];
    if(ranges && !ranges.split(',').some(range=>{
      const limits=range.trim().replace(/^U\+/i,'').split('-');
      const low=parseInt(limits[0].replaceAll('?','0'),16),high=parseInt((limits[1]||limits[0]).replaceAll('?','F'),16);
      return !Number.isFinite(low)||!Number.isFinite(high)||hasPoint(low,high);
    }))return '';
    let face=match[0];
    for(const url of face.matchAll(/url\(['"]?(\/assets\/[^)'"\s]+)['"]?\)/g))face=face.replace(url[0],`url(${await resource(url[1],'font/woff2')})`);
    return face;
  }));
  let index=0;
  return css.replace(/@font-face\s*\{[^}]*\}/g,()=>replacements[index++]);
}
