<script lang="ts">
  import { ui } from "./ui.svelte";
  import doc from "./schema";
  import { symbolFor } from "./signifiers";
  import { type Spread } from "./shared";

  const openCount = $derived(doc.current.entries.filter((item) => item.type === "task" || item.type === "scheduled").length);
</script>

<article class="exportPage" aria-label={ui.spread === "daily" ? "Exported daily rapid log" : "Exported monthly index"}>
  <span class="ribbon" aria-hidden="true"></span>
  {#if ui.spread === "daily"}
    <div class="titleRow">
      <h1 class="title">{doc.current.date.trim() || "Rapid log"}</h1>
      <span class="pageNumWrap">pg. {doc.current.dailyPage.trim() || "—"}</span>
    </div>
    <ul class="list">
      {#each doc.current.entries as item (item.$id)}
        <li class="row" data-complete={item.type === "complete"}>
          <span class="exportStar" aria-hidden="true">{item.star ? "*" : ""}</span>
          <span class="exportMark" data-type={item.type}>{symbolFor(item.type)}</span>
          <span class="exportText">{item.text.trim() || "Untitled"}</span>
        </li>
      {:else}
        <li class="empty"><h2>A blank page.</h2></li>
      {/each}
    </ul>
    <footer class="foot">
      <span>{doc.current.entries.length} bullets · {openCount} open</span>
      <span>Rapid log</span>
    </footer>
  {:else}
    <div class="titleRow">
      <h1 class="title">{doc.current.monthTitle.trim() || "Monthly index"}</h1>
      <span class="pageNumWrap">pg. {doc.current.monthlyPage.trim() || "—"}</span>
    </div>
    <ul class="list">
      {#each doc.current.monthlyLog as item (item.$id)}
        <li class="monthRow">
          <span class="monthDay">{String(item.day).padStart(2, "0")}</span>
          <span class="monthWeekday">{item.weekday.trim() || "—"}</span>
          <span class="monthText">{item.text.trim() || ""}</span>
        </li>
      {:else}
        <li class="empty"><h2>No days indexed.</h2></li>
      {/each}
    </ul>
    <footer class="foot">
      <span>{doc.current.monthlyLog.length} days indexed</span>
      <span>Monthly log</span>
    </footer>
  {/if}
</article>
