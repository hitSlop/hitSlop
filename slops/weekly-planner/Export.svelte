<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { minutes, label, duration, tasksByTime } from "./schedule";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();
</script>

<article class="exportPage">
  <header class="header">
    <div class="identity">
      <span class="eyebrow">Weekly planner</span>
      <h1 class="weekTitle">{snapshot.week || "This week"}</h1>
    </div>
    <p>{snapshot.focus}</p>
  </header>
  <div class="exportDays">
    {#each (preview ? (snapshot.days).slice(0, 7) : snapshot.days) as day (day.$id)}
      <section class="exportDay">
        <h2>{day.name} <span class="date">{day.date}</span></h2>
        {#each (preview ? (tasksByTime(day.tasks)).slice(0, 3) : tasksByTime(day.tasks)) as task (task.$id)}
          {@const start = minutes(task.time)}
          <div class="exportBlock" data-color={task.color} data-done={task.done}>
            <strong>{task.done ? "✓ " : ""}{task.title || "Untitled"}</strong><span class="blockTime">{start === null ? "Anytime" : `${label(start)} – ${label(Math.min(1440, start + duration(task)))}`}</span>
          </div>
        {:else}
          <p class="description">Open day</p>
        {/each}
      </section>
    {/each}
  </div>
</article>
