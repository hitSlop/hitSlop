<script lang="ts">
  import doc from "./schema";
  import { minutes, label, duration, tasksByTime } from "./schedule";
</script>

<article class="exportPage">
  <header class="header">
    <div class="identity">
      <span class="eyebrow">Weekly planner</span>
      <h1 class="weekTitle">{doc.current.week || "This week"}</h1>
    </div>
    <p>{doc.current.focus}</p>
  </header>
  <div class="exportDays">
    {#each doc.current.days as day (day.$id)}
      <section class="exportDay">
        <h2>{day.name} <span class="date">{day.date}</span></h2>
        {#each tasksByTime(day.tasks) as task (task.$id)}
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
