<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import type { CaptureProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  let { snapshot }: CaptureProps<typeof schema> = $props();
  const complete = $derived(snapshot.tasks.filter(task => task.done).length);
</script>

<main class="tiny-paper" style="background:var(--tiny-surface)">
  <div class="tiny-eyebrow">Little checklist</div>
  <h1 class="tiny-title">{snapshot.title || "Your checklist"}</h1>
  <p class="tiny-summary">{complete} of {snapshot.tasks.length} done. One thing at a time.</p>
  <ul class="tiny-list">
    {#each snapshot.tasks as task (task.$id)}
      <li class="tiny-row" data-done={task.done}>
        <span class="tiny-check" data-state={task.done ? "checked" : "unchecked"}>{task.done ? "✓" : ""}</span>
        <span class="tiny-text">{task.text}</span>
      </li>
    {/each}
  </ul>
  <footer class="tiny-footer">Your list, at your pace.</footer>
</main>
