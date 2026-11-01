<script lang="ts">
  import "@hitslop/pop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "@hitslop/pop/svelte";
  import type schema from "./schema";
  let { snapshot }: CaptureProps<typeof schema> = $props();
  const complete = $derived(snapshot.tasks.filter(task => task.done).length);
</script>

<main class="pop-paper" style="background:var(--pop-surface)">
  <div class="pop-eyebrow">Little checklist</div>
  <h1 class="pop-title">{snapshot.title || "Your checklist"}</h1>
  <p class="pop-summary">{complete} of {snapshot.tasks.length} done. One thing at a time.</p>
  <ul class="pop-list">
    {#each snapshot.tasks as task (task.$id)}
      <li class="pop-row" data-done={task.done}>
        <span class="pop-check" data-state={task.done ? "checked" : "unchecked"}>{task.done ? "✓" : ""}</span>
        <span class="pop-text">{task.text}</span>
      </li>
    {/each}
  </ul>
  <footer class="pop-footer">Your list, at your pace.</footer>
</main>
