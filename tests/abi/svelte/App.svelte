<script lang="ts">
  // Frozen consumer of the Svelte adapter. Checks run after mount; any failure makes
  // the export snippet throw, so native open/export of a sealed copy fails loudly.
  import { onMount, tick } from "svelte";
  import { Slop, bindText, bindValue, useDocument } from "@hitslop/document/svelte";
  import { capture } from "@hitslop/document/capture";
  import { attachments } from "@hitslop/document/attachments";
  import schema from "./schema";

  const doc = useDocument(schema);
  let failure = $state<string | null>(null);
  let passed = $state(0);
  let titleInput: HTMLInputElement;
  let doneInput: HTMLInputElement;
  let shown: HTMLParagraphElement;
  function check(name: string, ok: boolean) {
    if (!ok) throw new Error(`Svelte ABI check failed: ${name}`);
    passed++;
  }
  const finished = new Promise<void>((resolve) => {
    onMount(async () => {
      try {
        check("snapshot is reactive", shown.textContent === doc.current.title);
        doc.fields.title.replace(doc.current.title + " (svelte)");
        await tick();
        check("render follows edits", shown.textContent === doc.current.title);
        check("bindText renders", titleInput.value === doc.current.title);
        titleInput.value += "!";
        titleInput.dispatchEvent(new Event("input"));
        await doc.flush();
        check("bindText writes", doc.current.title.endsWith("!"));
        doneInput.checked = !doneInput.checked;
        doneInput.dispatchEvent(new Event("change"));
        check("bindValue writes", doc.current.done === doneInput.checked);
        const before = doc.current;
        try {
          doc.change((tx) => {
            tx.fields.title.replace("never");
            throw new Error("abort");
          });
        } catch {}
        check("change is atomic", doc.current === before);
        const id = doc.change((tx) => tx.fields.rows.insert({ name: "svelte", done: true }).id);
        check("at() over rows", doc.at(doc.current.rows.find((row) => row.$id === id)!) !== undefined);
        check("issues and capacity", Array.isArray(doc.issues) && doc.full === false);
        check("capture", typeof capture.isRenderer() === "boolean");
        check("attachments", Array.isArray(await attachments.list()));
        await doc.flush();
        check("saved", doc.status === "saved");
      } catch (error) {
        failure = String(error);
      }
      resolve();
    });
  });
  // Capture waits for the checks before rendering the export view.
  onMount(() => capture.onPrepare(() => finished));
  function verified() {
    if (failure) throw new Error(failure);
    return `ABI 1 (Svelte): ${passed} checks passed`;
  }
</script>

<Slop>
  <main>
    <p bind:this={shown}>{doc.current.title}</p>
    <input aria-label="Title" bind:this={titleInput} use:bindText={doc.fields.title} />
    <input aria-label="Done" type="checkbox" bind:this={doneInput} use:bindValue={doc.fields.done} />
  </main>
  {#snippet exportView()}<main>{verified()}</main>{/snippet}
</Slop>
