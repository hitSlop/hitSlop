import { tick } from "svelte";
export async function verifyNative(adapter: any, call: any) {
  const results: string[] = [];
  const assert = (ok: unknown, message: string) => {if(!ok)throw new Error(message);};
  const title = () => document.querySelector('textarea')!;
  async function reset(value:string) {
    await adapter.drain();
    await adapter.submit([{type:'splice',path:['title'],index:0,deleteCount:adapter.current().title.length,text:value}]);
    await tick();
  }
  function input(element: HTMLInputElement|HTMLTextAreaElement, value:string, start=value.length,end=start) {
    element.value=value;element.setSelectionRange(start,end);
    element.dispatchEvent(new InputEvent('input',{bubbles:true,inputType:'insertText'}));
  }
  for (const delay of [0,20,100,500]) {
    adapter.delay(delay);await reset('abc');
    input(title(),'abcX');input(title(),'abcXY');input(title(),'abcXYZ');
    await adapter.command([{type:'splice',path:['title'],index:0,deleteCount:0,text:'R'}]);
    await adapter.drain();await tick();
    assert(adapter.current().title==='RabcXYZ',`draft ancestry at ${delay}: ${adapter.current().title}`);
    assert(title().value==='RabcXYZ' && title().selectionStart===7,`DOM/caret at ${delay}`);
    results.push(`rapid input + command prefix / ${delay}ms`);
  }
  adapter.delay(0);
  await reset('a😀é中');input(title(),'a🚀é中',3);await adapter.drain();
  assert(adapter.current().title==='a🚀é中' && title().selectionStart===3,'UTF16 replacement');
  input(title(),'aé中',1);await adapter.drain();assert(adapter.current().title==='aé中','backspace surrogate pair');
  input(title(),'a日本語é中',4);await adapter.drain();assert(adapter.current().title==='a日本語é中','selection/paste');
  results.push('Unicode replacement/backspace/paste');
  await reset('abc');
  title().dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));
  input(title(),'abcに');input(title(),'abc日本');
  assert(adapter.current().title==='abc','sent during composition');
  await adapter.command([{type:'splice',path:['title'],index:0,deleteCount:0,text:'遠'}]);
  assert(title().value==='abc日本','remote publication replaced composing draft');
  title().dispatchEvent(new CompositionEvent('compositionend',{bubbles:true,data:'日本'}));
  await adapter.drain();assert(adapter.current().title==='遠abc日本','composition ancestry');
  results.push('synthetic composition with concurrent command');
  await tick();const updates=(globalThis as any).spikeRowUpdates;
  const last=adapter.current().rows.at(-1);
  await adapter.submit([{type:'set',path:['rows',{id:last.$id},'done'],value:!last.done}]);await tick();
  assert((globalThis as any).spikeRowUpdates-updates===1,`single-row edit updated ${(globalThis as any).spikeRowUpdates-updates} rows`);
  results.push('single-row edit updates exactly one rendered row');
  const first=adapter.current().rows[0].$id;
  const row=document.querySelector<HTMLInputElement>('input[aria-label="Row text"]')!;
  row.focus();adapter.delay(100);input(row,row.value+'x');
  await adapter.submit([{type:'remove',path:['rows'],id:first}]);await tick();await adapter.drain();
  assert(!adapter.current().rows.some((r:any)=>r.$id===first),'focused row resurrected');
  assert(document.activeElement!==row,'removed field retained focus');results.push('focused row deletion');
  adapter.delay(0);await reset('save me');await call('failSave');
  input(title(),'save me!');await adapter.drain();
  let failed=false;try{await adapter.flush();}catch{failed=true;}
  assert(failed && title().value==='save me!','failed save lost DOM draft or falsely succeeded');
  await call('retrySave');results.push('failed save retains text; retry succeeds');
  await reset('abc');
  return {cases:results,systemIME:'manual gate pending; synthetic composition is not a system IME test'};
}
