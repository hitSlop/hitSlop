<script lang="ts">
  import doc from "./schema";
  import Check from "@lucide/svelte/icons/check";
  import { QUADRANTS, type QuadrantKey } from "./shared";
  import { type Task, type Zone } from "./schema";

  const inbox = $derived(doc.current.tasks.filter((task) => task.zone === "inbox"));

  const totalActive = $derived(doc.current.tasks.filter((task) => !task.done).length);

  const q2Active = $derived(doc.current.tasks.filter((task) => task.zone === "q2" && !task.done).length);

  const q2Ratio = $derived(totalActive > 0 ? Math.round((q2Active / totalActive) * 100) : 0);

  function tasksIn(zone: Zone): Task[] {
    return doc.current.tasks.filter((task) => task.zone === zone);
  }
</script>

<article class="exportBlotter" aria-label="Exported Eisenhower matrix {doc.current.title}">
  <header class="letterhead">
    <div class="titleGroup">
      <h1 class="title">{doc.current.title.trim() || "Priority Desk Blotter"}</h1>
      <p class="date">{doc.current.date.trim() || "Undated"}</p>
    </div>
    <div class="leverage" aria-label="Q2 leverage {q2Ratio} percent">
      <span class="leverageRow">
        <span>Q2 Leverage</span>
        <span class="leverageValue">{q2Ratio}%</span>
      </span>
      <span class="meter" aria-hidden="true"><span class="meterFill" style:width="{q2Ratio}%"></span></span>
    </div>
    <span class="activeCount">{totalActive} open</span>
  </header>

  <div class="axis" aria-hidden="true">
    <span>◀ Urgent</span>
    <span>Not urgent ▶</span>
  </div>

  <div class="exportMatrix">
    {#each QUADRANTS as q (q.key)}
      <section class="exportQuadrant" data-quad={q.key} aria-label="{q.title} tasks">
        <div class="quadHeader">
          <div class="quadBadge">
            <span class="roman" data-quad={q.key}>{q.num}</span>
            <div class="quadText">
              <h2 class="quadTitle">{q.title}</h2>
              <span class="quadSub">{q.subtitle}</span>
            </div>
          </div>
          <span class="quadTag">{q.tag}</span>
        </div>
        <ul class="exportList">
          {#each tasksIn(q.key) as task (task.$id)}
            <li class="row" data-done={task.done}>
              <span data-checkbox-root data-state={task.done ? "checked" : "unchecked"}>{#if task.done}<Check size={10} strokeWidth={3} />{/if}</span>
              <span class="exportText">{task.text.trim() || "Untitled task"}</span>
            </li>
          {:else}
            <li class="empty">{q.empty}</li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>

  <footer class="tray">
    <div class="trayHead">
      <div class="trayTitle">
        <h3 class="trayHeading">Holding Pen</h3>
      </div>
      <span class="trayCount">{inbox.length} parked</span>
    </div>
    {#if inbox.length > 0}
      <ul class="trayList" aria-label="Holding pen">
        {#each inbox as item (item.$id)}
          <li class="inboxRow">
            <span class="exportText">{item.text.trim() || "Untitled task"}</span>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="empty">Nothing waiting to be stamped.</p>
    {/if}
  </footer>
</article>
