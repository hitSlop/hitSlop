<script lang="ts">
  import doc from "./schema";
  import { formatDate, layout, toLabel } from "./schedule";
  import { bookedMinutesOf } from "./schedule";

  const booked = $derived(bookedMinutesOf(doc.current.blocks));
</script>

<article class="exportPage" aria-label="Exported daily planner">
  <header class="header">
    <span class="eyebrow">DAILY / PLANNER</span>
    <div class="headerMain">
      <h1 class="title">{formatDate(doc.current.date, { weekday: "long" })}</h1>
      <span class="dateNumber">{formatDate(doc.current.date, { day: "2-digit" })}</span>
    </div>
    <p>{formatDate(doc.current.date, { month: "long", day: "numeric", year: "numeric" })}</p>
  </header>
  <section class="priorities" style="max-height:none;overflow:visible">
    <h2 class="heading">Make room for</h2>
    <ol class="priorityList">
      {#each doc.current.priorities as priority, index (priority.$id)}
        <li class="priority">
          <span class="checkbox">{priority.done ? "✓" : index + 1}</span>
          <span style:text-decoration={priority.done ? "line-through" : "none"}>{priority.text || "—"}</span>
        </li>
      {/each}
    </ol>
  </section>
  <div class="scheduleHead">
    <h2 class="heading">Your day</h2>
    <span class="meta">{Math.floor(booked / 60)}h {booked % 60}m planned</span>
  </div>
  <div class="exportList">
    {#each layout(doc.current.blocks) as item (item.block.$id)}
      <section class="exportBlock" data-kind={item.block.kind}>
        <span class="meta">{toLabel(item.start)}–{toLabel(item.end)} · {item.block.kind}</span>
        <h3 class="exportTitle">{item.block.title || "Untitled block"}</h3>
      </section>
    {:else}
      <p>No time blocks yet.</p>
    {/each}
  </div>
  <section class="exportNotes">
    <h2 class="heading">Day notes</h2>
    {doc.current.notes || "Nothing noted."}
  </section>
</article>
