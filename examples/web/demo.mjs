// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT
import { createUi, createWorld } from './load.mjs';
import { browserKey, exactText, physicalCode, PressOwnership } from './boundary.mjs';
import { ClipboardRead } from './clipboard.mjs';
import { loadFonts } from './fonts.mjs';

const element=(tag,attributes={},text='')=>{
  const node=document.createElement(tag);
  for(const [key,value] of Object.entries(attributes)) node.setAttribute(key,value);
  node.textContent=text;
  return node;
};

export async function createDemo(root,{backend='auto',clipboard=globalThis.navigator?.clipboard,autoAccept=true}={}) {
  const families=await loadFonts();
  const app=await createUi();
  root.style.fontFamily=families;
  const host=new DomHost(root,app,{clipboard,autoAccept});
  try {
    await document.fonts.ready;
    host.world=await createWorld(host.canvas,Math.max(1,host.canvas.clientWidth),Math.max(1,host.canvas.clientHeight),backend);
    host.status.textContent=`${host.world.backend()} · shared Rust state`;
    host.install();
    app.receive('Welcome. Type a message; Enter and Send use the same shared action.');
    app.receive('Unicode: é · 👩🏽‍💻 · שלום · مرحبا');
    host.render();
    return host;
  } catch(error) { host.dispose(); throw error; }
}

class DomHost {
  constructor(root,app,options) {
    this.root=root;this.app=app;this.options=options;this.abort=new AbortController();
    this.clipboard=new ClipboardRead();this.presses=new PressOwnership();this.worldKeys=new Set();
    this.stamp=null;this.composition=null;this.inputToken=null;this.awaitFreshEdit=false;this.presentedScroll=null;this.rendering=false;this.dirty=false;this.disposed=false;this.accepted='0';
    root.classList.add('shared-demo');
    this.canvas=element('canvas',{'aria-label':'Neutral Tilewright world'});
    this.worldState=element('p',{class:'world-state','aria-live':'off'},'World controls released');
    const worldPanel=element('section',{class:'world-panel','aria-label':'Map viewport'});
    worldPanel.append(this.canvas,this.worldState);
    this.history=element('ol',{class:'chat-history',role:'log','aria-label':'Chat history','aria-live':'polite',tabindex:'0'});
    this.input=element('input',{type:'text','aria-label':'Message',autocomplete:'off',spellcheck:'false',dir:'auto'});
    this.send=element('button',{type:'button'},'Send');
    this.pasteButton=element('button',{type:'button'},'Paste');
    this.modalButton=element('button',{type:'button'},'Open modal');
    this.older=element('button',{type:'button','aria-label':'Older messages'},'↑');
    this.newer=element('button',{type:'button','aria-label':'Newer messages'},'↓');
    const composer=element('div',{class:'composer'});composer.append(this.input,this.send,this.pasteButton);
    const toolbar=element('div',{class:'toolbar'});toolbar.append(this.older,this.newer,this.modalButton);
    this.status=element('p',{class:'status',role:'status'});
    const panel=element('section',{class:'chat-panel','aria-label':'Shared chat'});panel.append(this.history,composer,toolbar,this.status);
    this.dialog=element('dialog',{'aria-labelledby':'shared-modal-title'});
    const title=element('h2',{id:'shared-modal-title'},'Shared modal');
    const description=element('p',{},'World controls stay cancelled while this modal owns focus.');
    this.closeButton=element('button',{type:'button'},'Close modal');this.dialog.append(title,description,this.closeButton);
    root.replaceChildren(worldPanel,panel,this.dialog);
  }
  on(node,event,handler,options={}) { node.addEventListener(event,handler,{...options,signal:this.abort.signal}); }
  snapshot() { return JSON.parse(this.app.snapshot()); }
  target(state=this.snapshot()) { return [state.generation,state.control.epoch,state.textRevision]; }
  report(error) { if(!this.disposed)this.status.textContent=String(error?.message??error); }
  change(operation) {
    if(this.disposed)return false;
    let success=true;
    try {operation();this.acceptPending();}catch(error){success=false;this.report(error);}
    this.render();return success;
  }
  acceptPending() {
    if(!this.options.autoAccept)return;
    // One monotonically acknowledged batch cursor; no unbounded retry store.
    for(let count=0;count<16;count+=1){
      const batch=JSON.parse(this.app.peek_actions());if(!batch)return;
      if(BigInt(batch.id)>BigInt(this.accepted)){
        for(const action of batch.actions)this.app.receive(action.text);
        this.accepted=batch.id;
      }
      this.app.ack_actions(batch.id);
    }
  }
  focusedWidget(node) {
    if(node===this.input)return '1';
    if(node===this.send)return '2';
    return node&&this.root.contains(node)?'3':'0';
  }
  focusFor(node) { this.app.focus_widget(this.focusedWidget(node)); }
  syncSelection() {
    if(this.composition||document.activeElement!==this.input)return;
    const state=this.snapshot();if(state.control.focused!=='1')return;
    const anchor=this.input.selectionDirection==='backward'?this.input.selectionEnd:this.input.selectionStart;
    const focus=this.input.selectionDirection==='backward'?this.input.selectionStart:this.input.selectionEnd;
    if(anchor===state.anchor&&focus===state.focus)return;
    if(!this.stamp)throw new Error('Selection waits for a committed layout');
    this.app.select(this.stamp,anchor,focus,...this.target(state));
  }
  cancelComposition() {
    const session=this.composition;
    this.composition=null;this.inputToken=null;
    if(!session)return;
    this.awaitFreshEdit=true;
    const state=this.snapshot();
    // Cancel only the exact preedit this session produced. A newer independent
    // owner/revision is never cancelled merely because an old callback arrived.
    if(state.composing&&state.generation===session.owner[0]&&state.textRevision===session.owner[2]&&state.control.focused==='1')this.app.cancel_composition(...this.target(state));
  }
  reconcileComposition() {
    if(this.composition&&this.composition.owner.some((value,index)=>value!==this.target()[index]))this.cancelComposition();
  }
  finishInput() {
    const state=this.snapshot();
    const origin=this.composition?.owner??this.inputToken;
    if(!origin||state.control.focused!=='1'||(origin&&(origin[0]!==state.generation||origin[1]!==state.control.epoch||(this.composition&&origin[2]!==state.textRevision)))){
      this.cancelComposition();this.inputToken=null;this.render();return;
    }
    const direction=this.input.selectionDirection;
    const anchor=direction==='backward'?this.input.selectionEnd:this.input.selectionStart;
    const focus=direction==='backward'?this.input.selectionStart:this.input.selectionEnd;
    const target=origin;
    const session=this.composition;this.composition=null;this.inputToken=null;
    if(!this.change(()=>this.app.replace(exactText(this.input.value),anchor,focus,...target))&&session){this.composition=session;this.cancelComposition();this.render();}
  }
  install() {
    this.on(this.input,'focus',()=>this.change(()=>this.focusFor(this.input)));
    this.on(this.root,'focusin',event=>{if(event.target!==this.input)this.change(()=>this.focusFor(event.target));});
    this.on(this.root,'focusout',event=>{
      if(event.target===this.input){this.cancelComposition();this.inputToken=null;this.clipboard.cancel();}
      this.change(()=>this.focusFor(event.relatedTarget));
    });
    this.on(this.input,'pointerdown',event=>{
      try {if(!this.stamp)throw new Error('Layout unavailable');this.app.validate_displayed(this.stamp);this.awaitFreshEdit=false;}
      catch(error){event.preventDefault();this.report(error);}
    });
    this.on(document,'selectionchange',()=>{
      if(document.activeElement===this.input&&!this.composition)this.change(()=>this.syncSelection());
    });
    this.on(this.input,'beforeinput',event=>{
      if(this.awaitFreshEdit){event.preventDefault();this.render();return;}
      if(event.isComposing||this.composition)return;
      try {this.syncSelection();this.render();this.inputToken=this.target();}
      catch(error){event.preventDefault();this.report(error);this.render();}
    });
    this.on(this.input,'input',event=>{if(!event.isComposing&&(!this.composition||this.composition.ending))this.finishInput();});
    this.on(this.input,'compositionstart',()=>{
      try {this.awaitFreshEdit=false;this.syncSelection();this.composition={owner:this.target(),ending:false};}
      catch(error){this.report(error);}
    });
    this.on(this.input,'compositionupdate',event=>{
      if(!this.composition)return;
      const state=this.snapshot();
      if(this.composition.owner.some((value,index)=>value!==this.target(state)[index])){this.cancelComposition();this.render();return;}
      this.change(()=>{this.app.preedit(exactText(event.data),...this.target(state));this.composition.owner=this.target();});
    });
    this.on(this.input,'compositionend',()=>{
      if(!this.composition)return;
      const session=this.composition;session.ending=true;
      queueMicrotask(()=>{if(this.composition===session&&!this.disposed)this.finishInput();});
    });
    // Pointer activation keeps the composer selection for Send and Paste.
    for(const button of [this.send,this.pasteButton])this.on(button,'pointerdown',event=>event.preventDefault());
    this.on(this.send,'click',()=>{if(!this.composition)this.change(()=>this.app.submit());});
    this.on(this.pasteButton,'click',()=>{void this.paste();});
    this.on(this.modalButton,'click',()=>this.change(()=>this.app.modal(true)));
    this.on(this.closeButton,'click',()=>this.change(()=>this.app.modal(false)));
    this.on(this.dialog,'cancel',event=>{event.preventDefault();this.change(()=>this.app.modal(false));});
    this.on(this.dialog,'close',()=>{if(this.snapshot().control.modal)this.change(()=>this.app.modal(false));});
    this.on(this.older,'click',()=>this.change(()=>this.app.scroll(1)));
    this.on(this.newer,'click',()=>this.change(()=>this.app.scroll(-1)));
    this.on(this.history,'wheel',event=>{event.preventDefault();this.change(()=>this.app.scroll(Math.sign(event.deltaY)));},{passive:false});
    this.on(window,'keydown',event=>this.key(event,true),{capture:true});
    this.on(window,'keyup',event=>this.key(event,false),{capture:true});
    this.on(window,'blur',()=>{this.cancelComposition();this.inputToken=null;this.clipboard.cancel();this.change(()=>this.app.window_focused(false));});
    this.on(window,'focus',()=>this.change(()=>{this.app.window_focused(true);this.focusFor(document.activeElement);}));
    this.on(document,'visibilitychange',()=>{this.cancelComposition();this.clipboard.cancel();this.change(()=>{this.app.suspend(document.hidden);if(!document.hidden)this.focusFor(document.activeElement);});});
    const invalidate=()=>this.change(()=>{this.app.invalidate_surface();this.stamp=null;});
    this.resize=new ResizeObserver(invalidate);this.resize.observe(this.root);
    this.on(window,'resize',invalidate);
    this.on(window,'scroll',event=>{if(event.target===this.history)this.render();else invalidate();},{capture:true});
  }
  key(event,pressed) {
    if(this.disposed)return;
    const inComposer=event.target===this.input;
    const composing=Boolean(this.composition)||event.isComposing||event.keyCode===229;
    const commandEvent=(!inComposer&&event.target!==this.send)||composing?{code:event.code,key:'',repeat:event.repeat,isComposing:composing}:event;
    try {
      const result=browserKey(this.app,commandEvent,pressed);
      if(pressed&&event.key==='Escape'&&this.composition)this.cancelComposition();
      if(pressed&&!event.repeat&&!composing&&result.keyEffects&&event.key!=='Escape')this.awaitFreshEdit=false;
      // Routing consumption does not suppress normal browser editing/shortcuts.
      if(result.consumed)event.stopPropagation();
      const reserved=(event.ctrlKey||event.metaKey)&&['l','r','w','t','n','q'].includes(event.key.toLowerCase());
      if(pressed&&!composing&&event.key==='Enter'&&(inComposer||event.target===this.send))event.preventDefault();
      if(pressed&&event.repeat&&inComposer&&!composing&&!reserved&&!result.keyEffects)event.preventDefault();
      const owner=pressed?this.presses.down(event.code,!result.consumed):this.presses.up(event.code);
      if(owner==='world'){if(pressed)this.worldKeys.add(event.code);else this.worldKeys.delete(event.code);}
      this.acceptPending();this.render();
    }catch(error){this.report(error);this.render();}
  }
  async paste() {
    if(this.disposed)return;
    this.input.focus();this.awaitFreshEdit=false;
    try {this.syncSelection();await this.clipboard.paste(this.app,this.options.clipboard);}
    catch(error){this.report(error);}
    if(!this.disposed)this.render();
  }
  render() {
    if(this.disposed||!this.world)return;
    if(this.rendering){this.dirty=true;return;}
    this.rendering=true;
    try {
      for(let attempt=0;attempt<8;attempt+=1){
        this.dirty=false;this.reconcileComposition();
        const state=this.snapshot();
        for(const key of this.presses.observe(state.control))this.worldKeys.delete(key);
        this.worldState.textContent=this.worldKeys.size?`World controls: ${[...this.worldKeys].join(', ')}`:'World controls released';
        if(!this.composition){
          const value=state.composing?state.displayText:state.text;
          if(this.input.value!==value)this.input.value=value;
          if(document.activeElement===this.input){
            const start=Math.min(state.anchor,state.focus),end=Math.max(state.anchor,state.focus),direction=state.anchor>state.focus?'backward':'forward';
            if(this.input.selectionStart!==start||this.input.selectionEnd!==end||(start!==end&&this.input.selectionDirection!==direction))this.input.setSelectionRange(start,end,direction);
          }
        }
        this.input.disabled=state.control.modal||state.control.suspended||!state.composerVisible;
        this.send.disabled=this.pasteButton.disabled=this.input.disabled;
        const visible=state.messages.slice(0,Math.max(0,state.messages.length-state.scroll));
        // Preserve the accessibility log when its shared text has not changed.
        const prior=[...this.history.children];
        if(prior.length!==visible.length||prior.some((node,index)=>node.textContent!==visible[index])){
          const atBottom=this.history.scrollHeight-this.history.clientHeight-this.history.scrollTop<2;
          const scrollTop=this.history.scrollTop;
          this.history.replaceChildren(...visible.map(text=>element('li',{dir:'auto'},text)));
          this.history.scrollTop=atBottom||state.scroll!==this.presentedScroll?this.history.scrollHeight:scrollTop;
        }
        this.presentedScroll=state.scroll;
        this.older.disabled=state.scroll>=Math.max(0,state.messages.length-1);this.newer.disabled=state.scroll===0;
        if(state.control.modal&&!this.dialog.open)this.dialog.showModal();
        if(!state.control.modal&&this.dialog.open){this.dialog.close();this.input.focus();}
        if(this.dirty||this.snapshot().revision!==state.revision)continue;
        const width=Math.max(1,Math.round(this.canvas.clientWidth));const height=Math.max(1,Math.round(this.canvas.clientHeight));
        this.world.render(width,height);
        const bounds=this.root.getBoundingClientRect();
        // Browser layout has committed here; all bounds and scroll positions are
        // measured synchronously before acknowledging this exact shared revision.
        this.input.getBoundingClientRect();this.history.getBoundingClientRect();
        const stamp=this.app.prepare(bounds.width,bounds.height,window.devicePixelRatio||1);
        if(JSON.parse(stamp).revision!==state.revision){this.app.discard_prepared(stamp);continue;}
        try {this.app.ack_presented(stamp);this.stamp=stamp;}catch(error){this.app.discard_prepared(stamp);throw error;}
        return;
      }
      throw new Error('Focus did not settle within the bounded layout commit');
    }catch(error){this.stamp=null;this.report(error);}finally{this.rendering=false;}
  }
  dispose() {
    if(this.disposed)return;
    this.disposed=true;this.abort.abort();this.resize?.disconnect();this.clipboard.cancel();
    this.app.suspend(true);this.app.invalidate_surface();this.stamp=null;this.worldKeys.clear();
    this.world?.free();this.app.free();this.root.replaceChildren();
  }
}
