import init, {render_preview_browser} from './pkg/mdpdf-app.js';
const ready=init({module_or_path:new URL('./pkg/mdpdf-app.wasm',self.location.href)});
void ready.catch(()=>{});
let queue=[],running=false;
self.onmessage=event=>{
  const job=event.data;
  // Superseded previews can be dropped; explicit HTML exports must complete.
  if(job.channel==='preview')queue=queue.filter(pending=>pending.channel!=='preview');
  queue.push(job);
  if(running)return;
  running=true;
  void (async()=>{
    try{
      await ready;
      await new Promise(resolve=>setTimeout(resolve,0));
      while(queue.length){
        const {id,payload}=queue.shift();
        try{self.postMessage({id,result:JSON.parse(render_preview_browser(payload))});}
        catch(error){self.postMessage({id,error:String(error)});}
        await new Promise(resolve=>setTimeout(resolve,0));
      }
    }catch(error){for(const {id}of queue)self.postMessage({id,error:String(error),fatal:true});queue=[];}
    finally{running=false;}
  })();
};
