<script lang="ts">
  import { EditableText } from "hitslop/svelte";
  import doc from "./schema";
  let hidden = $state(false);
</script>
<main data-editor>
  <EditableText field={doc.fields.title} label="Title" />
  <button data-toggle-view aria-pressed={hidden} onclick={() => hidden = !hidden}>Toggle local view</button>
  <section data-rows hidden={hidden}>
    {#each doc.current.tasks as task (task.$id)}
      {@const row = doc.at(task)}
      <div data-row>
        <input type="checkbox" aria-label={task.text} bind:checked={row.done.value} />
        <EditableText field={row.text} label={task.$id} />
      </div>
    {/each}
  </section>
</main>
