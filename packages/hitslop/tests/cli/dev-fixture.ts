import { cp, writeFile } from "node:fs/promises";
import { join } from "node:path";

// Dedicated integration fixture: accepted values, row IDs, and one mounted UI
// detect broken HMR ownership through the generated Svelte entry.
export async function createFixture(repository: string, source: string) {
  await cp(join(repository, "tests/apps/document"), source, { recursive: true });
  await writeFile(join(source, "probe.svelte.ts"), "export const local = $state({count: 0});\n");
  await writeFile(
    join(source, "Child.svelte"),
    `
<script lang="ts">
  import {bindText, getDocument} from 'hitslop/svelte';
  import schema from './schema';
  const doc = getDocument(schema);
</script>
<input aria-label="Document title" use:bindText={doc.fields.title} />
<output data-title>{doc.current.title}</output>
`,
  );
  await writeFile(
    join(source, "App.svelte"),
    `
<script lang="ts">
  import './styles.css';
  import Circle from '@lucide/svelte/icons/circle';
  import schema from './schema';
  import {local} from './probe.svelte';
  import {attachments, getDocument, getEnvironment} from 'hitslop/svelte';
  import Child from './Child.svelte';
  const doc = getDocument(schema);
  const environment = getEnvironment();
  const addRow = ({text}: {text: string}) => doc.fields.tasks.insert({text});
  (globalThis as any).__devProbe = {doc, addRow, attachments};
  let error = $state('');
  async function add() {
    try { await addRow({text:'Accepted row'}); await doc.flush(); }
    catch (e) { error = String(e); }
  }
</script>
  <main data-probe>
    <output data-environment>{environment.current.platform}/{environment.current.device}/{environment.current.layout}</output>
    <h1>Revision zero</h1><Circle aria-label="Dependency icon" />
    <Child />
    <button onclick={add}>Add row</button>
    <output data-count>{doc.current.tasks.length}</output>
    <output data-row-ids>{doc.current.tasks.map(row => row.$id).join(',')}</output>
    <button onclick={() => local.count++}>Local {local.count}</button>
    <output data-error>{error}</output>
  </main>
<style>
  h1 { color: rgb(20, 40, 60); }
  :global(html[data-slop-platform="macos"][data-slop-device="desktop"][data-slop-layout="compact"]) [data-environment] { color: rgb(11, 22, 33); }
</style>
`,
  );
}
