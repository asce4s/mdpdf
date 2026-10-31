// Small, dependency-free Markdown transformations shared by the editor and tests.
export function headings(source) {
  const result=[]; let fence=null, front=source.startsWith('---\n'), offset=0;
  const lines=source.split('\n');
  for(let i=0;i<lines.length;i++) {
    const line=lines[i], marker=line.match(/^ {0,3}(`{3,}|~{3,})/);
    if(front) { if(i>0 && /^(---|\.\.\.)\s*$/.test(line)) front=false; offset+=line.length+1; continue; }
    if(marker) { if(!fence) fence=marker[1]; else if(marker[1][0]===fence[0]&&marker[1].length>=fence.length) fence=null; offset+=line.length+1; continue; }
    if(!fence) {
      const atx=line.match(/^ {0,3}(#{1,6})\s+(.+?)\s*#*\s*$/);
      if(atx) result.push({level:atx[1].length,text:atx[2],from:offset,line:i+1});
      else if(line.trim()&&i+1<lines.length&&/^ {0,3}(=+|-+)\s*$/.test(lines[i+1])) result.push({level:lines[i+1].trim()[0]==='='?1:2,text:line.trim(),from:offset,line:i+1});
    }
    offset+=line.length+1;
  }
  return result;
}
export function splitRow(line) {
  line=line.trim().replace(/^\|/,'').replace(/(?<!\\)\|$/,'');
  const cells=[];let value='',code=0;
  for(let i=0;i<line.length;i++) {
    if(line[i]==='\\'&&i+1<line.length) {value+=line[i]+line[++i];continue;}
    if(line[i]==='`') {let n=1;while(line[i+1]==='`'){n++;i++;}code=code===n?0:code||n;value+='`'.repeat(n);continue;}
    if(line[i]==='|'&&!code){cells.push(value.trim());value='';}else value+=line[i];
  }
  cells.push(value.trim());return cells;
}
export function tableMarkdown(rows,align=[]) {
  const width=Math.max(1,...rows.map(row=>row.length));
  const row=values=>'| '+Array.from({length:width},(_,i)=>String(values[i]||'').replace(/\r?\n/g,' ').replace(/(?<!\\)\|/g,'\\|')).join(' | ')+' |';
  const header=rows[0]||Array(width).fill('');
  return [row(header),row(Array.from({length:width},(_,i)=>({left:':---',center:':---:',right:'---:'}[align[i]]||'---'))),...rows.slice(1).map(row)].join('\n');
}
export function parseTable(text) {
  const lines=text.trim().split('\n');if(lines.length<2)return null;
  const separator=splitRow(lines[1]);if(!separator.every(cell=>/^:?-{3,}:?$/.test(cell)))return null;
  return {rows:[splitRow(lines[0]),...lines.slice(2).map(splitRow)],align:separator.map(cell=>cell.startsWith(':')?(cell.endsWith(':')?'center':'left'):(cell.endsWith(':')?'right':''))};
}
export function tsvTable(text) {
  if(!text.includes('\t')||!text.trim())return null;
  const rows=text.replace(/\r\n/g,'\n').replace(/\n$/,'').split('\n').map(line=>line.split('\t'));
  return tableMarkdown(rows);
}
