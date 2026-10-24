import {execFileSync} from 'node:child_process';
import {cpSync,existsSync,mkdirSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
const output=resolve('target/browser');
execFileSync('npm',['run','vendor'],{stdio:'inherit'});
execFileSync('cargo',['leptos','build','--release','--frontend-only'],{
  stdio:'inherit',env:{...process.env,LEPTOS_SITE_ROOT:output},
});
if(!existsSync(output+'/pkg/mdpdf-app.wasm'))throw new Error('The frontend build did not create '+output+'/pkg/mdpdf-app.wasm');
mkdirSync(output+'/assets/vendor',{recursive:true});
cpSync('assets/fonts',output+'/assets/fonts',{recursive:true});
for(const name of ['mermaid','katex'])cpSync('assets/vendor/'+name,output+'/assets/vendor/'+name,{recursive:true});
writeFileSync(output+'/index.html',`<!doctype html>
<html lang="en"><head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="theme-color" content="#171b19">
<title>mdpdf — Markdown to PDF in your browser</title>
<link rel="icon" href="./favicon.svg" type="image/svg+xml">
<link rel="stylesheet" href="./app.css">
<script type="module" src="./browser-entry.js"></script>
</head><body><p id="loading" role="status">Loading the editor…</p></body></html>\n`);
writeFileSync(output+'/.nojekyll','');
console.log('Static browser app built in '+output+'; deploy these files to any static host.');
