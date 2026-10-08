import { cp, writeFile } from "node:fs/promises";
import { join } from "node:path";

// Dedicated integration fixture: accepted values, row IDs, and one mounted UI
// detect broken HMR ownership through the generated Svelte entry.
export async function createFixture(repository: string, source: string) {
  await cp(join(repository, "packages/hitslop/templates/checklist"), source, { recursive: true });
  await writeFile(join(source, "probe.svelte.ts"), "export const local = $state({count: 0});\n");
  await writeFile(
    join(source, "Child.svelte"),
    `
<script lang="ts">
  import {bindText} from 'hitslop/svelte';
  import doc from './schema';
</script>
<input aria-label="Document title" use:bindText={doc.fields.title} />
<output data-title>{doc.current.title}</output>
`,
  );
  await writeFile(
    join(source, "App.svelte"),
    `
<script lang="ts">
  import Circle from '@lucide/svelte/icons/circle';
  import doc from './schema';
  import {local} from './probe.svelte';
  import {addTask} from './commands';
  import {attachments} from 'hitslop/svelte';
  (globalThis as any).__devProbe = {doc, addTask, attachments};
  import Child from './Child.svelte';
  let error = $state('');
  async function add() {
    try { await addTask({text:'Accepted row'}); await doc.flush(); }
    catch (e) { error = String(e); }
  }
</script>
  <main data-probe>
    <h1>Revision zero</h1><Circle aria-label="Dependency icon" />
    <Child />
    <button onclick={add}>Add row</button>
    <output data-count>{doc.current.tasks.length}</output>
    <output data-row-ids>{doc.current.tasks.map(row => row.$id).join(',')}</output>
    <button onclick={() => local.count++}>Local {local.count}</button>
    <output data-error>{error}</output>
  </main>
<style>h1 { color: rgb(20, 40, 60); }</style>
`,
  );
}
