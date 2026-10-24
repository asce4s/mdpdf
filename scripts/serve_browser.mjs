// Development/test file server. The deployed app needs only these static files.
import {createServer} from 'node:http';
import {createReadStream} from 'node:fs';
import {stat} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
const option=(name,fallback)=>{const index=process.argv.indexOf(name);return index<0?fallback:process.argv[index+1];};
const root=resolve(option('--dir','target/browser'));
const port=Number(option('--port',process.env.PORT||'4173'));
const base='/'+option('--base','/').replace(/^\/+|\/+$/g,'');
const prefix=base==='/'?'/':base+'/';
const mime={'.html':'text/html; charset=utf-8','.js':'text/javascript; charset=utf-8','.css':'text/css; charset=utf-8','.wasm':'application/wasm','.woff2':'font/woff2','.svg':'image/svg+xml','.json':'application/json','.png':'image/png'};
const server=createServer(async(request,response)=>{
  if(!['GET','HEAD'].includes(request.method)){response.writeHead(405);response.end();return;}
  let pathname;
  try{pathname=decodeURIComponent(new URL(request.url,'http://localhost').pathname);}catch{response.writeHead(400);response.end();return;}
  if(base!=='/'&&pathname===base){response.writeHead(302,{location:prefix});response.end();return;}
  if(!pathname.startsWith(prefix)){response.writeHead(404);response.end();return;}
  const relative=pathname.slice(prefix.length)||'index.html';
  const file=resolve(root,relative);
  if(!file.startsWith(root+sep)){response.writeHead(403);response.end();return;}
  try{
    const info=await stat(file);if(!info.isFile())throw new Error('Not a file');
    const etag='"'+info.size+'-'+Math.trunc(info.mtimeMs)+'"';
    if(request.headers['if-none-match']===etag){response.writeHead(304);response.end();return;}
    response.writeHead(200,{'content-type':mime[extname(file)]||'application/octet-stream','content-length':info.size,'cache-control':'no-cache',etag});
    if(request.method==='HEAD')response.end();else createReadStream(file).pipe(response);
  }catch{response.writeHead(404);response.end('Not found');}
});
server.listen(port,'127.0.0.1',()=>console.log(`Static preview: http://127.0.0.1:${port}${prefix}`));
