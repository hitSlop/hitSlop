<script lang="ts">
  import { getDocument, attachments } from 'hitslop/svelte';
  import schema from './schema';
  import photo from '../../abi/owner-svelte/media/swatch.png';
  const doc=getDocument(schema);
  let draft=$state('');
  let error=$state('');
  const add=async(text:string)=>{await doc.fields.rows.insert({text});};
  const attach=async(file:File)=>attachments.import<typeof schema.descriptor>(file,(tx,ref)=>{tx.fields.files.insert({hash:ref.id,name:ref.name,mimeType:ref.mimeType,bytes:ref.byteLength});});
</script>
<main>
  <h1>Sharing fixture</h1>
  <img src={photo} width="32" height="32" alt="Private app image"/>
  <label>Add a file <input aria-label="Add a file" type="file" onchange={async(e)=>{const file=e.currentTarget.files?.[0];if(file)try{await attach(file);}catch(e){error=String(e);}}}/></label>
  <ul>{#each doc.current.files as file(file.$id)}<li data-file={file.hash}>{file.name} ({file.bytes} bytes){#if file.mimeType.startsWith('image/')}<img alt={file.name} src={attachments.url(file.hash)} width="64" height="64"/>{/if}</li>{/each}</ul>
  <p><strong data-count>{doc.current.rows.length}</strong> saved rows</p>
  <form onsubmit={async(e)=>{e.preventDefault();try{await add(draft);draft='';}catch(e){error=String(e);}}}>
    <input aria-label="New row" bind:value={draft}/><button>Add row</button>
  </form>
  <output data-error>{error}</output>
  <ol>{#each doc.current.rows as row(row.$id)}<li data-row>{row.text}</li>{/each}</ol>
</main>
<style>
  @font-face{font-family:Fixture;src:url('../../abi/owner-svelte/media/sample.ttf')}
  :global(body){margin:0;background:var(--slop-surface,#faf8f1);color:#242a2b;font:16px Fixture,system-ui}
  main{padding:24px}h1{font-size:24px}form{display:flex;gap:8px}input{min-width:0;flex:1;padding:10px}
  button{background:#237b63;color:white;border:0;border-radius:6px;padding:10px}li{overflow-wrap:anywhere;margin:8px 0}
</style>
