// Portable ZIP files use stored entries: readable by ordinary ZIP tools, without dependencies.
const encoder=new TextEncoder(), decoder=new TextDecoder();
export function crc32(bytes){let crc=0xffffffff;for(const byte of bytes){crc^=byte;for(let i=0;i<8;i++)crc=(crc>>>1)^((crc&1)?0xedb88320:0);}return (crc^0xffffffff)>>>0;}
export function zip(entries){const chunks=[],central=[];let offset=0;
  for(const [name,value] of entries){const path=encoder.encode(name),data=typeof value==='string'?encoder.encode(value):new Uint8Array(value),crc=crc32(data),header=new Uint8Array(30+path.length),v=new DataView(header.buffer);v.setUint32(0,0x04034b50,true);v.setUint16(4,20,true);v.setUint16(6,0x800,true);v.setUint32(14,crc,true);v.setUint32(18,data.length,true);v.setUint32(22,data.length,true);v.setUint16(26,path.length,true);header.set(path,30);chunks.push(header,data);
    const c=new Uint8Array(46+path.length),cv=new DataView(c.buffer);cv.setUint32(0,0x02014b50,true);cv.setUint16(4,20,true);cv.setUint16(6,20,true);cv.setUint16(8,0x800,true);cv.setUint32(16,crc,true);cv.setUint32(20,data.length,true);cv.setUint32(24,data.length,true);cv.setUint16(28,path.length,true);cv.setUint32(42,offset,true);c.set(path,46);central.push(c);offset+=header.length+data.length;
  }
  const size=central.reduce((n,c)=>n+c.length,0),end=new Uint8Array(22),v=new DataView(end.buffer);v.setUint32(0,0x06054b50,true);v.setUint16(8,central.length,true);v.setUint16(10,central.length,true);v.setUint32(12,size,true);v.setUint32(16,offset,true);return new Blob([...chunks,...central,end],{type:'application/zip'});
}
export function unzip(buffer){const bytes=new Uint8Array(buffer),v=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),files=new Map();let offset=0,total=0;
  if(bytes.length>70*1024*1024)throw new Error('Bundle exceeds 70 MB');
  while(offset+4<=bytes.length&&v.getUint32(offset,true)===0x04034b50){
    if(offset+30>bytes.length)throw new Error('Truncated bundle');
    const flags=v.getUint16(offset+6,true),method=v.getUint16(offset+8,true),crc=v.getUint32(offset+14,true),size=v.getUint32(offset+18,true),raw=v.getUint32(offset+22,true),length=v.getUint16(offset+26,true),extra=v.getUint16(offset+28,true),start=offset+30+length+extra;
    if(flags&9||method!==0||size!==raw)throw new Error('Use an uncompressed mdpdf ZIP bundle');
    if(start+size>bytes.length)throw new Error('Truncated bundle');
    const name=decoder.decode(bytes.subarray(offset+30,offset+30+length));
    if(!name||name.startsWith('/')||name.includes('\\')||name.split('/').some(p=>p==='..'||p==='.')||files.has(name))throw new Error('Invalid bundle path');
    total+=size;if(total>64*1024*1024||files.size>=102)throw new Error('Bundle content exceeds limits');
    const data=bytes.slice(start,start+size);if(crc32(data)!==crc)throw new Error('Bundle checksum failed');files.set(name,data);offset=start+size;
  }
  if(!files.size||offset+4>bytes.length||v.getUint32(offset,true)!==0x02014b50)throw new Error('Invalid ZIP bundle');return files;
}
