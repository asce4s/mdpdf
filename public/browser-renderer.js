import {abortError,embedCss,nonce,resource,readRemoteImage} from './browser-assets.js';
export class BrowserRenderer {
  requests=new Map(); serial=0; worker=null; remote=new Map(); remoteRequests=new Map(); remoteFailures=new Set(); currentMarkdown='';
  constructor(assets,onAssetsChanged) {
    this.assets=assets;this.onAssetsChanged=onAssetsChanged;
    try {
      this.worker=new Worker(new URL('./browser-worker.js',import.meta.url),{type:'module'});
      this.worker.onmessage=({data})=>{
        if(data.fatal){this.failWorker(new Error(data.error));return;}
        const request=this.requests.get(data.id);if(!request)return;
        this.requests.delete(data.id);
        if(data.error)request.reject(new Error(data.error));else request.resolve(data.result);
      };
      this.worker.onerror=event=>{event.preventDefault();this.failWorker(new Error('Preview worker unavailable'));};
    }catch{this.worker=null;}
  }
  failWorker(error) {
    this.worker?.terminate();this.worker=null;
    for(const request of this.requests.values())request.reject(error);
    this.requests.clear();
  }
  async parse(payload,signal) {
    if(signal?.aborted)throw abortError();
    const serialized=JSON.stringify(payload);
    if(!this.worker)return window.mdpdfBrowserRender(serialized);
    try{return await new Promise((resolve,reject)=>{
      const id=++this.serial;
      const abort=()=>{this.requests.delete(id);reject(abortError());};
      this.requests.set(id,{resolve:result=>{signal?.removeEventListener('abort',abort);resolve(result);},reject:error=>{signal?.removeEventListener('abort',abort);reject(error);}});
      signal?.addEventListener('abort',abort,{once:true});
      this.worker.postMessage({id,payload:serialized,channel:signal?'preview':'export'});
    });}catch(error){
      if(signal?.aborted||error.name==='AbortError')throw abortError();
      if(!this.worker)return window.mdpdfBrowserRender(serialized);
      throw error;
    }
  }
  async render(markdown,theme,{includeShell=false,signal,remoteImages=true}={}) {
    this.currentMarkdown=markdown;
    const images={...this.assets.imageMap(markdown)};
    for(const [url,image]of this.remote)if(markdown.includes(url))images[url]=image.dataUrl;
    const result=await this.parse({markdown,theme,images,include_shell:includeShell,nonce:includeShell?nonce():''},signal);
    if(signal?.aborted)throw abortError();
    if(remoteImages)this.resolveRemote(result.unresolvedImages);
    const characters=result.body.replace(/\bsrc="data:[^"]*"/g,'')+JSON.stringify(result.metadata)+JSON.stringify(result.theme);
    result.css=await embedCss(result.css,characters);
    if(result.hasMath)result.mathScript=await resource('/assets/vendor/katex/katex.min.js','text/javascript');
    result.fonts=[...this.assets.assets.values()].filter(asset=>asset.kind==='font'&&[result.theme.typography.body_font,result.theme.typography.heading_font,result.theme.typography.mono_font,result.theme.diagrams.font_family].includes('assetfont-'+asset.id)).map(asset=>({family:'assetfont-'+asset.id,url:asset.dataUrl}));
    if(signal?.aborted)throw abortError();
    if(includeShell)result.html=await this.document(result);
    return result;
  }
  resolveRemote(urls) {
    for(const [url,request]of this.remoteRequests)if(!urls.includes(url)){request.cancelled=true;request.controller.abort();this.remoteRequests.delete(url);}
    for(const url of urls){
      if(!/^https?:\/\//i.test(url)||this.remote.has(url)||this.remoteRequests.has(url)||this.remoteFailures.has(url))continue;
      const controller=new AbortController();
      const timer=setTimeout(()=>controller.abort(),8000);
      const promise=readRemoteImage(url,controller.signal).then(image=>{
        const bytes=[...this.remote.values()].reduce((sum,value)=>sum+value.size,0);
        if(bytes+image.size>64*1024*1024)throw new Error('Remote document images exceed 64 MB');
        this.remote.set(url,image);
        if(this.currentMarkdown.includes(url))this.onAssetsChanged();
      }).catch(()=>{if(!this.remoteRequests.get(url)?.cancelled)this.remoteFailures.add(url);}).finally(()=>{clearTimeout(timer);if(this.remoteRequests.get(url)?.controller===controller)this.remoteRequests.delete(url);});
      this.remoteRequests.set(url,{controller,promise});
    }
  }
  async document(result) {
    let html=result.shell;
    const oldCss=html.split('id="mdpdf-theme">')[1].split('</style>')[0];
    html=html.replace('id="mdpdf-theme">'+oldCss+'</style>','id="mdpdf-theme">'+result.css+'</style>');
    html=html.replace('src="/assets/vendor/mermaid/mermaid.min.js"','src="'+await resource('/assets/vendor/mermaid/mermaid.min.js','text/javascript')+'"');
    if(result.hasMath)html=html.replace('src="/assets/vendor/katex/katex.min.js"','src="'+result.mathScript+'"');
    html=html.replace("font-src 'self'","font-src data:");
    // A document link must not navigate the live frame away from its trusted
    // runtime, which receives the user's subsequent preview updates.
    const documentNonce=html.match(/<script nonce="([^"]+)"/)?.[1];
    const links="if(window.parent!==window)document.addEventListener('click',event=>{const link=event.target.closest?.('a[href]');const url=link?.getAttribute('href');if(url&&!url.startsWith('#')){event.preventDefault();window.parent.postMessage({mdpdf:true,type:'open-link',url},'*');}});";
    html=html.replace('</body>',`<script nonce="${documentNonce}">${links}</script></body>`);
    if(result.fonts.length){
      const css=result.fonts.map(font=>`@font-face{font-family:"${font.family}";src:url(${font.url});font-weight:100 900;font-style:normal;font-display:block;}`).join('\n');
      html=html.replace('</head>',`<style id="mdpdf-custom-fonts">${css}</style></head>`);
    }
    return html;
  }
  async exportHtml(markdown,theme) {
    const result=await this.render(markdown,theme,{includeShell:true});
    if(this.remoteRequests.size){await Promise.all([...this.remoteRequests.values()].map(request=>request.promise));return (await this.render(markdown,theme,{includeShell:true,remoteImages:false})).html;}
    return result.html;
  }
}
