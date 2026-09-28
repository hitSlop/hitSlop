<script lang="ts">
  import type {FixtureRow} from './types';
  let {row, candidate, submit, binding}:{row:FixtureRow;candidate:string;submit:(ops:any[])=>Promise<any>;binding:any}=$props();
  $effect(()=>{row; (globalThis as any).spikeRowUpdates=((globalThis as any).spikeRowUpdates??0)+1;});
</script>
<label><input type="checkbox" checked={row.done} onchange={()=>submit([{type:'set',path:['rows',{id:row.$id},'done'],value:!row.done}])}/><input aria-label="Row text" value={candidate === "rust-core" ? undefined : row.text} use:binding={["rows",{id:row.$id},"text"]}/></label>
<style>label{display:flex;gap:10px}</style>
