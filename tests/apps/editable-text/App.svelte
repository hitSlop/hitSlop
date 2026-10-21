<script lang="ts">
  import { EditableText, getDocument } from "hitslop/svelte";
  import schema from "./schema";
  import InteractionProbe from "./InteractionProbe.svelte";
  const doc = getDocument(schema);
  let destination: HTMLInputElement;
  // A test-only bridge to the real public handle; no document semantics are mocked.
  (globalThis as any).setFixtureText = (text: string) => doc.fields.first.set(text);
</script>
<main>
  <EditableText class="text" field={doc.fields.first} label="First text" onenter={() => destination.focus()} />
  <EditableText class="text" field={doc.fields.second} label="Second text" />
  <input bind:this={destination} aria-label="Focus destination" />
  <InteractionProbe />
</main>
<style>
  main { width: 260px; padding: 20px; }
  :global(.text) { font: 18px/1.5 monospace; margin-bottom: 20px; }
</style>
