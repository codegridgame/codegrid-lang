export function memoryMaximum(bytes) {
 const data=bytes instanceof Uint8Array?bytes:new Uint8Array(bytes);let offset=8;
 function leb(){let n=0,shift=0,b;do{b=data[offset++];n+=(b&127)*2**shift;shift+=7;}while(b&128);return n;}
 while(offset<data.length){const id=data[offset++],size=leb(),end=offset+size;if(id===5){if(leb()!==1)throw new Error('Expected one memory');if(leb()!==1)throw new Error('Memory must declare maximum');leb();return BigInt(leb())*65536n;}offset=end;}throw new Error('Memory section missing');
}
