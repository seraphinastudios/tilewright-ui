// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT
const MANIFEST_SHA='94b81a712fb41a66eb744c88ae2ed312cc5d0ec1a491e9821fcc278d99410f13';
const hex=bytes=>Array.from(new Uint8Array(bytes),byte=>byte.toString(16).padStart(2,'0')).join('');
async function verified(url,length,digest) {
  const response=await fetch(url);if(!response.ok)throw new Error(`Missing explicit font asset: ${url.pathname}`);
  const bytes=await response.arrayBuffer();
  if(bytes.byteLength!==length||hex(await crypto.subtle.digest('SHA-256',bytes))!==digest)throw new Error('Font asset integrity mismatch');
  return bytes;
}
let fonts;
export function loadFonts() {
  return fonts??=(async()=>{
    const base=new URL('./assets/font-pack/',import.meta.url);
    const manifestBytes=await verified(new URL('manifest.json',base),423554,MANIFEST_SHA);
    const manifest=JSON.parse(new TextDecoder().decode(manifestBytes));
    if(manifest.fonts.length!==17)throw new Error('Unexpected font pack');
    const faces=await Promise.all(manifest.fonts.map(async(font,index)=>{
      const bytes=await verified(new URL(font.path,base),font.bytes,font.sha256);
      return new FontFace(`Shared Font ${index}`,bytes).load();
    }));
    for(const face of faces)document.fonts.add(face);
    return faces.map(face=>`"${face.family}"`).join(',');
  })();
}
