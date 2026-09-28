<script lang="ts">
  import Row from "./Row.svelte";
  import { untrack, onMount } from "svelte";
  import type { Adapter, FixtureView, ViewController } from "./types";
  import type { Operation } from "../contract";
  let { adapter, initial, candidate, register }:{adapter:any;initial:FixtureView;candidate:string;register:(view:ViewController)=>void}=$props();
  let view:FixtureView=$state.raw(untrack(()=>initial)), draft=$state(untrack(()=>initial.title)), error=$state('');
  let submitted=$state(0), accepted=$state(0);
  let queue=Promise.resolve();
  async function submit(ops:Operation[]){
    const result=await adapter.submit(ops);
    view=adapter.current();
    return result;
  }
  let titleElement: HTMLTextAreaElement;
  function binding(element: HTMLTextAreaElement | HTMLInputElement, path: any[]) {
    return adapter.bind?.(element,path,(e:unknown)=>error=String(e)) ?? {};
  }
  onMount(()=>adapter.subscribe?.(()=>view=adapter.current()));
  function input(event:Event){
    if (candidate === 'rust-core') return;
    const next=(event.target as HTMLTextAreaElement).value, before=draft;draft=next;
    let prefix=0;while(prefix<before.length&&prefix<next.length&&before[prefix]===next[prefix])prefix++;
    const sequence=++submitted;
    queue=queue.then(async()=>{
      await submit([{type:'splice',path:['title'],index:prefix,deleteCount:before.length-prefix,text:next.slice(prefix)}]);
      accepted=sequence;
      if(accepted===submitted)draft=view.title;
    }).catch(e=>{error=String(e);throw e});
  }
  untrack(()=>register({submit, state:()=>view, setDraft:(v:string)=>draft=v, drain:()=>adapter.drain?.() ?? queue, pending:()=>adapter.pending?.() ?? submitted-accepted, type:(text:string)=>{
    if(candidate==='rust-core'){titleElement.value+=text;titleElement.setSelectionRange(titleElement.value.length,titleElement.value.length);titleElement.dispatchEvent(new InputEvent('input',{bubbles:true,inputType:'insertText',data:text}));}
    else input({target:{value:draft+text}} as unknown as Event);
  }}));
</script>
<main>
  <h1>{candidate}</h1>
  <p>Isolated engine placement experiment</p>
  <textarea bind:this={titleElement} use:binding={["title"]} aria-label="Document title" value={candidate === "rust-core" ? undefined : draft} oninput={input}></textarea>
  {#if view.body}<div class="editor" contenteditable="false" aria-label="Rich text projection">{view.body.text}</div>{/if}
  {#if view.taps !== undefined}<button onclick={()=>submit([{type:'increment',path:['taps'],amount:1}])}>Count {view.taps}</button>{/if}
  <p role="status">{error || `${accepted}/${submitted} text edits accepted`}</p>
  <section>
    {#each (view.rows.length>40?[...view.rows.slice(0,39),...view.rows.slice(-1)]:view.rows) as row (row.$id)}
      <Row {row} {candidate} {submit} {binding}/>
    {/each}
  </section>
</main>
<style>
  :global(body){font:15px system-ui;margin:0;background:#f4f3ef;color:#202830}main{padding:24px}h1{font-size:20px}textarea{width:90%;font:inherit;min-height:64px}section{display:grid;gap:8px}.editor{padding:12px;background:white}
</style>
