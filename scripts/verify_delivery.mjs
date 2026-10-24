// Real CLI/web layout parity, first-page visual goldens, and timed workload.
import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {resolve} from 'node:path';
import assert from 'node:assert/strict';
import {PNG} from 'pngjs';
const root=process.env.MDPDF_BASE_URL || 'http://127.0.0.1:3000';
const out=resolve('target/verification');mkdirSync(out,{recursive:true});mkdirSync('e2e/golden',{recursive:true});
const theme=(await (await fetch(root+'/api/themes')).json())[0].theme;
const command=(name,args)=>execFileSync(name,args,{encoding:'utf8',maxBuffer:32*1024*1024});
const cli=resolve(process.env.MDPDF_CLI || 'target/debug/mdpdf');
const report=[];
for(const file of readdirSync('crates/mdpdf-core/tests/fixtures').filter(x=>x.endsWith('.md')&&(!process.env.MDPDF_FIXTURES||process.env.MDPDF_FIXTURES.split(',').includes(x.slice(0,-3)))).sort()){
 const name=file.slice(0,-3),source=resolve('crates/mdpdf-core/tests/fixtures',file),markdown=readFileSync(source,'utf8');
 const response=await fetch(root+'/api/export',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({markdown,theme})});
 assert.equal(response.status,200,await (!response.ok?response.text():Promise.resolve('')));
 const web=out+'/'+name+'.web.pdf',local=out+'/'+name+'.cli.pdf';writeFileSync(web,Buffer.from(await response.arrayBuffer()));
 command(cli,[source,'-o',local]);
 const pages=pdf=>Number(command('pdfinfo',[pdf]).match(/^Pages:\s+(\d+)/m)[1]);
 assert.equal(pages(web),pages(local),name+' pages');
 const text=pdf=>command('pdftotext',['-layout',pdf,'-']).replace(/\s+/g,'');
 assert.equal(text(web),text(local),name+' text');
 const differences=[];
 for(const page of name==='math-cover'?[1,2]:[1]){
  const imageName=name+(page===1?'':'-page'+page);
  command('pdftocairo',['-f',String(page),'-l',String(page),'-singlefile','-scale-to','1100','-png',web,out+'/'+imageName]);
  const actual=readFileSync(out+'/'+imageName+'.png'),golden='e2e/golden/'+imageName+'.png';
  if(process.env.UPDATE_GOLDENS==='1')writeFileSync(golden,actual);
  const a=PNG.sync.read(actual),b=PNG.sync.read(readFileSync(golden));assert.equal(a.width,b.width);assert.equal(a.height,b.height);
  let different=0;for(let i=0;i<a.data.length;i+=4){if([0,1,2].some(c=>Math.abs(a.data[i+c]-b.data[i+c])>16))different++;}
  const ratio=different/(a.width*a.height);assert.ok(ratio<=.005,`${imageName}: ${(ratio*100).toFixed(3)}% pixels differ`);differences.push({page,ratio});
 }
 report.push({fixture:name,pages:pages(web),pixelDifference:differences});console.log(name+': layout and golden pass');
}
const markdown=Array.from({length:30},(_,i)=>`# Benchmark page ${i+1}\n\nA representative document with selectable text and consistent type.\n\n${i<20?'```mermaid\nflowchart LR\n A[Write]-->B[Review]-->C[Publish]\n```\n':''}${i<29?'<!-- pagebreak -->':''}`).join('\n\n');
const started=performance.now();const response=await fetch(root+'/api/export',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({markdown,theme})});assert.equal(response.status,200);const pdf=out+'/benchmark.pdf';writeFileSync(pdf,Buffer.from(await response.arrayBuffer()));const elapsed=performance.now()-started;
assert.equal(Number(command('pdfinfo',[pdf]).match(/^Pages:\s+(\d+)/m)[1]),30);assert.ok(elapsed<10000,`30 pages/20 diagrams: ${elapsed.toFixed()}ms`);
console.log(`30 pages / 20 diagrams: ${elapsed.toFixed()}ms`);writeFileSync(out+'/report.json',JSON.stringify({fixtures:report,benchmarkMs:elapsed},null,2)+'\n');
