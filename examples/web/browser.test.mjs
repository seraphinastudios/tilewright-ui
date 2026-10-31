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
    response.setHeader('Content-Type',path.endsWith('.wasm')?'application/wasm':path.endsWith('.html')?'text/html':path.endsWith('.css')?'text/css':path.endsWith('.ttf')?'font/ttf':path.endsWith('.json')?'application/json':'text/javascript'); response.end(data);
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


test('DOM presenter commits native editing, modal focus, scrolling and clipboard safely', {timeout:120000}, async () => {
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  let browser;
  try {
    browser=await chromium.launch({headless:true,args:['--no-sandbox','--enable-unsafe-webgpu','--enable-features=Vulkan','--use-vulkan=swiftshader','--use-angle=swiftshader','--use-webgpu-adapter=swiftshader']});
    const page=await browser.newPage();const errors=[];page.on('pageerror',error=>errors.push(error.message));
    await page.goto(`http://127.0.0.1:${server.address().port}/index.html?backend=webgl2`);
    await page.waitForFunction(()=>globalThis.demo);
    const input=page.getByRole('textbox',{name:'Message'});
    await input.fill('A👩🏽‍💻');await input.press('End');await input.press('Backspace');
    assert.equal(await input.inputValue(),'A');
    await input.press('Control+a');await page.keyboard.insertText('é שלום مرحبا नमस्ते 日本語 한국어');
    assert.equal(await page.evaluate(()=>demo.snapshot().text),'é שלום مرحبا नमस्ते 日本語 한국어');
    await input.fill('<img src=x onerror=alert(1)>');await page.getByRole('button',{name:'Send',exact:true}).click();
    assert.equal(await page.locator('.chat-history li').last().textContent(),'<img src=x onerror=alert(1)>');
    assert.equal(await page.locator('.chat-history img').count(),0);
    assert.equal(await input.inputValue(),'');
    await input.fill('draft');
    await input.evaluate(node=>{
      node.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));
      node.value='draft漢字';node.setSelectionRange(node.value.length,node.value.length);
      node.dispatchEvent(new CompositionEvent('compositionupdate',{data:'漢字',bubbles:true}));
      node.dispatchEvent(new InputEvent('input',{inputType:'insertCompositionText',data:'漢字',isComposing:true,bubbles:true}));
    });
    await page.keyboard.down('Enter');
    assert.equal(await page.evaluate(()=>demo.app.peek_actions()),'null');
    assert.equal(await page.evaluate(()=>demo.snapshot().composing),true);
    await input.evaluate(node=>node.dispatchEvent(new CompositionEvent('compositionend',{data:'漢字',bubbles:true})));
    await page.waitForFunction(()=>demo.snapshot().text==='draft漢字'&&!demo.snapshot().composing);
    await page.getByRole('button',{name:'Send',exact:true}).click();
    assert.equal(await input.inputValue(),'draft漢字');
    await page.keyboard.up('Enter');
    await page.getByRole('button',{name:'Send',exact:true}).click();
    assert.equal(await input.inputValue(),'');
    await input.fill('empty preedit fence');
    await input.evaluate(node=>node.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true})));
    await page.keyboard.down('Enter');
    await input.evaluate(node=>node.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true})));
    await page.getByRole('button',{name:'Send',exact:true}).click();
    assert.equal(await input.inputValue(),'empty preedit fence');
    await page.keyboard.up('Enter');
    await page.getByRole('button',{name:'Send',exact:true}).click();
    assert.equal(await input.inputValue(),'');
    // A surface epoch change cancels its exact preedit and rejects trailing
    // events even after native focus still points at the same input element.
    await input.fill('anchor');
    await input.evaluate(node=>{
      node.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));
      node.value='anchor候補';node.setSelectionRange(node.value.length,node.value.length);
      node.dispatchEvent(new CompositionEvent('compositionupdate',{data:'候補',bubbles:true}));
    });
    await page.setViewportSize({width:1100,height:780});
    await page.waitForFunction(()=>!demo.snapshot().composing);
    await input.evaluate(node=>{
      node.value='late resize commit';node.setSelectionRange(node.value.length,node.value.length);
      node.dispatchEvent(new CompositionEvent('compositionend',{data:'候補',bubbles:true}));
      node.dispatchEvent(new InputEvent('input',{inputType:'insertFromComposition',bubbles:true}));
    });
    assert.equal(await input.inputValue(),'anchor');
    assert.equal(await page.evaluate(()=>demo.snapshot().text),'anchor');
    await input.press('End');await page.keyboard.type('x');
    assert.equal(await input.inputValue(),'anchorx');
    await input.evaluate(node=>{
      globalThis.retiredImeNode=node;
      node.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));
      node.value='anchorx候補';node.setSelectionRange(node.value.length,node.value.length);
      node.dispatchEvent(new CompositionEvent('compositionupdate',{data:'候補',bubbles:true}));
    });
    await input.press('Escape');
    assert.equal(await page.evaluate(()=>retiredImeNode.isConnected),false);
    await input.click();
    await page.evaluate(()=>{
      retiredImeNode.value='old native target';
      retiredImeNode.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true}));
      retiredImeNode.dispatchEvent(new InputEvent('beforeinput',{inputType:'insertText',bubbles:true,cancelable:true}));
      retiredImeNode.dispatchEvent(new InputEvent('input',{inputType:'insertText',bubbles:true}));
    });
    await input.evaluate(node=>{
      node.value='late cancelled commit';node.setSelectionRange(node.value.length,node.value.length);
      node.dispatchEvent(new CompositionEvent('compositionend',{data:'候補',bubbles:true}));
      node.dispatchEvent(new InputEvent('beforeinput',{inputType:'insertFromComposition',bubbles:true,cancelable:true}));
      node.dispatchEvent(new InputEvent('input',{inputType:'insertFromComposition',bubbles:true}));
    });
    assert.equal(await page.evaluate(()=>demo.snapshot().composing),false);
    assert.equal(await input.inputValue(),'anchorx');
    await input.press('End');await page.keyboard.type('y');
    assert.equal(await input.inputValue(),'anchorxy');
    await page.getByRole('button',{name:'Open modal'}).click();
    assert.equal(await page.getByRole('dialog').isVisible(),true);
    assert.equal(await page.evaluate(()=>demo.snapshot().control.worldAllowed),false);
    await page.keyboard.press('Escape');
    await page.waitForFunction(()=>!document.querySelector('dialog').open);
    assert.equal(await input.evaluate(node=>node===document.activeElement),true);
    await page.evaluate(()=>demo.change(()=>{for(let i=0;i<30;i+=1)demo.app.receive(`line ${i}`);}));
    await page.getByRole('button',{name:'Older messages'}).click();
    assert.equal(await page.evaluate(()=>demo.snapshot().scroll),1);
    assert.equal(await page.locator('.chat-history li').last().textContent(),'line 28');
    await page.getByRole('button',{name:'Newer messages'}).click();
    assert.equal(await page.evaluate(()=>demo.snapshot().scroll),0);
    await page.evaluate(()=>demo.change(()=>demo.app.receive('wrapped '.repeat(200))));
    const history=page.getByRole('log',{name:'Chat history'});
    await history.focus();await history.press('PageUp');
    await page.waitForFunction(()=>{const node=document.querySelector('.chat-history');return node.scrollTop<node.scrollHeight-node.clientHeight-5;});
    await history.evaluate(node=>{node.scrollTop=0;});
    await page.waitForFunction(()=>document.querySelector('.chat-history').scrollTop===0);
    await page.evaluate(()=>demo.render());
    assert.equal(await history.evaluate(node=>node.scrollTop),0);
    await input.fill('unchanged');
    await page.evaluate(()=>{demo.options.clipboard={readText:async()=>{throw new Error('Permission denied');}};});
    await page.getByRole('button',{name:'Paste',exact:true}).click();
    await page.waitForFunction(()=>demo.status.textContent.includes('Permission denied'));
    assert.equal(await input.inputValue(),'unchanged');
    await page.evaluate(()=>{demo.options.clipboard={readText:()=>new Promise(resolve=>{globalThis.finishClipboard=resolve;})};});
    await page.getByRole('button',{name:'Paste',exact:true}).click();
    await input.fill('newer');await page.evaluate(()=>finishClipboard('stale'));
    await page.waitForFunction(()=>demo.status.textContent.includes('Stale'));
    assert.equal(await input.inputValue(),'newer');
    const reserved=await input.evaluate(node=>{
      const event=new KeyboardEvent('keydown',{key:'l',code:'KeyL',ctrlKey:true,bubbles:true,cancelable:true});node.dispatchEvent(event);return !event.defaultPrevented;
    });assert.equal(reserved,true);
    await page.locator('canvas').click();await page.keyboard.down('w');
    assert.match(await page.locator('.world-state').textContent(),/KeyW/);
    await input.click();await page.keyboard.up('w');
    assert.equal(await page.locator('.world-state').textContent(),'World controls released');
    const stale=await page.evaluate(()=>{const stamp=demo.stamp;demo.app.receive('pending patch');try{demo.app.validate_displayed(stamp);return false;}catch{return true;}finally{demo.render();}});
    assert.equal(stale,true);
    assert.deepEqual(errors,[]);
    console.log(JSON.stringify({dom:true,browser:await browser.version(),text:await input.inputValue(),fontFaces:await page.evaluate(()=>document.fonts.size)}));
    await page.evaluate(()=>demo.dispose());
  } finally {await browser?.close();await new Promise(resolve=>server.close(resolve));}
});
