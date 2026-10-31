// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import test from 'node:test';

const require = createRequire(process.env.PLAYWRIGHT_PACKAGE_JSON ?? '/opt/task-npm/package.json');
const { chromium } = require('playwright-core');
const directory = fileURLToPath(new URL('.', import.meta.url));
const server = createServer(async (request,response) => {
  const path = decodeURIComponent(new URL(request.url,'http://localhost').pathname);
  if (path.includes('..')) { response.writeHead(400).end(); return; }
  if (path === '/') { response.setHeader('Content-Type','text/html'); response.end('<!doctype html><html lang="en"><title>Shared UI boundary proof</title><body><canvas id="world" width="640" height="360"></canvas></body></html>'); return; }
  try {
    const data = await readFile(directory + path);
    response.setHeader('Content-Type',path.endsWith('.wasm') ? 'application/wasm' : 'text/javascript'); response.end(data);
  } catch { response.writeHead(404).end(); }
});

test('real Chromium executes shared WASM state and renderer backends', {timeout:120000}, async () => {
  await new Promise(resolve => server.listen(0,'127.0.0.1',resolve));
  let browser;
  try {
    browser = await chromium.launch({headless:true,args:['--no-sandbox','--enable-unsafe-webgpu','--enable-features=Vulkan','--use-vulkan=swiftshader','--use-angle=swiftshader','--use-webgpu-adapter=swiftshader']});
    const page = await browser.newPage();
    const errors=[]; page.on('pageerror',error=>errors.push(error.message));
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    const result = await page.evaluate(async () => {
      const {createUi,createWorld} = await import('./load.mjs');
      const {ClipboardRead} = await import('./clipboard.mjs');
      const {browserKey,physicalCode} = await import('./boundary.mjs');
      const app = await createUi(); app.focus(true);
      const state=()=>JSON.parse(app.snapshot());
      const replace=text=>{const s=state();app.replace(text,text.length,text.length,s.generation,s.control.epoch,s.textRevision);};
      const rejects=fn=>{try{fn();return false;}catch{return true;}};
      replace('e\u0301 👩🏽‍💻 אבג مرحبا');
      const before=state();
      const invalidSurrogateOffset=rejects(()=>app.replace('😀',1,1,before.generation,before.control.epoch,before.textRevision));
      const atomicInvalid=app.snapshot()===JSON.stringify(before);
      const malformedString=rejects(()=>app.receive("\ud800"));
      app.preedit('漢字',before.generation,before.control.epoch,before.textRevision);
      app.submit(); const composingDoesNotSubmit=app.peek_actions()==='null';
      replace('完成 👩🏽‍💻'); app.submit();
      const batch=app.peek_actions(); const stablePeek=app.peek_actions()===batch;
      app.ack_actions(JSON.parse(batch).id); app.ack_actions(JSON.parse(batch).id);
      const acknowledged=app.peek_actions()==='null';
      replace('old'); const stale=state(); replace('new');
      const staleRejected=rejects(()=>app.replace('overwrite',9,9,stale.generation,stale.control.epoch,stale.textRevision));
      const clipboard=new ClipboardRead();let complete;
      const pending=clipboard.paste(app,{readText:()=>new Promise(resolve=>{complete=resolve;})});
      replace('newer');complete('stale paste');
      let clipboardRejected=false;try{await pending;}catch{clipboardRejected=state().text==='newer';}
      const frame=app.prepare(640,360,1);
      const uncommitted=rejects(()=>app.validate_displayed(frame)); app.ack_presented(frame); app.validate_displayed(frame);
      app.invalidate_surface(); const retired=rejects(()=>app.ack_presented(frame));
      for(let i=0;i<16;i++){replace(`batch ${i}`);app.submit();}
      replace('preserved'); const full=app.snapshot(); const backpressure=rejects(()=>app.submit())&&app.snapshot()===full;
      const epoch=state().control.epoch; app.modal(true);app.modal(false); const cancellation=epoch!==state().control.epoch;
      const backends=[];
      for(const backend of ['webgl2','webgpu']) {
        const canvas=document.createElement('canvas');canvas.width=640;canvas.height=360;document.body.append(canvas);
        const world=await createWorld(canvas,640,360,backend);world.render(640,360);backends.push(world.backend());world.free();canvas.remove();
      }
      app.free();
      const keys=await createUi(); keys.focus(true);
      const physical=(code,key,pressed,repeat=false)=>browserKey(keys,{code,key,repeat},pressed);
      const keyReplace=text=>{const s=JSON.parse(keys.snapshot());keys.replace(text,text.length,text.length,s.generation,s.control.epoch,s.textRevision);};
      keyReplace('first');physical('Enter','Enter',true);
      keyReplace('second');physical('NumpadEnter','Enter',true);
      const first=JSON.parse(keys.peek_actions());keys.ack_actions(first.id);
      const second=JSON.parse(keys.peek_actions());
      const distinctEnters=first.actions[0].text==='first'&&second.actions[0].text==='second';
      physical('Enter','Enter',false);
      const independentRelease=physical('NumpadEnter','Enter',true,true).keyEffects;
      keys.modal(true);keys.modal(false);keys.focus(true);
      const quarantined=physical('NumpadEnter','Enter',true,true);
      const fencedEffects=quarantined.consumed&&!quarantined.keyEffects;
      let reads=0;let clipboardKeyRejected=false;
      try { await new ClipboardRead().pasteKey(keys,physicalCode('NumpadEnter'),{readText:async()=>{reads+=1;return 'forbidden';}}); }
      catch { clipboardKeyRejected=reads===0; }
      physical('NumpadEnter','Enter',false);
      const freshEffects=physical('KeyA','a',true).keyEffects;
      keys.free();
      return {invalidSurrogateOffset,atomicInvalid,malformedString,composingDoesNotSubmit,stablePeek,acknowledged,staleRejected,clipboardRejected,uncommitted,retired,backpressure,cancellation,distinctEnters,independentRelease,fencedEffects,clipboardKeyRejected,freshEffects,backends};
    });
    for (const [name,value] of Object.entries(result)) if(name!=='backends') assert.equal(value,true,name);
    assert.equal(result.backends.length,2); assert.deepEqual(errors,[]);
    console.log(JSON.stringify({browser:await browser.version(),...result}));
  } finally { await browser?.close(); await new Promise(resolve=>server.close(resolve)); }
});
