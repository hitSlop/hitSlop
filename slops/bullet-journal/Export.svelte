<script lang="ts">
  import { previewText } from "../_shared/preview";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { journalView } from "./journal";
  import { symbolFor } from "./signifiers";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  const { entries, openCount, days } = $derived(journalView(snapshot));
</script>

<article class="exportPage" aria-label="Exported bullet journal">
  <span class="ribbon" aria-hidden="true"></span>
  <section aria-label="Daily rapid log">
    <div class="titleRow">
      <h1 class="title">{snapshot.date.trim() || "Rapid log"}</h1>
      <span class="pageNumWrap">pg. {snapshot.dailyPage.trim() || "—"}</span>
    </div>
    <ul class="list">
      {#each (preview ? (entries).slice(0, 6) : entries) as item (item.$id)}
        <li class="row" data-complete={item.type === "complete"}>
          <span class="exportStar" aria-hidden="true">{item.star ? "*" : ""}</span>
          <span class="exportMark" data-type={item.type}>{symbolFor(item.type)}</span>
          <span class="exportText">{previewText(item.text.trim() || "Untitled", preview, 110)}</span>
        </li>
      {:else}
        <li class="empty"><h2>A blank page.</h2></li>
      {/each}
    </ul>
    <footer class="foot">
      <span>{entries.length} bullets · {openCount} open</span>
      <span>Rapid log</span>
    </footer>
  </section>
  <section aria-label="Monthly index">
    <div class="titleRow">
      <h1 class="title">{snapshot.monthTitle.trim() || "Monthly index"}</h1>
      <span class="pageNumWrap">pg. {snapshot.monthlyPage.trim() || "—"}</span>
    </div>
    <ul class="list">
      {#each (preview ? (days).slice(0, 4) : days) as item (item.$id)}
        <li class="monthRow">
          <span class="monthDay">{String(item.day).padStart(2, "0")}</span>
          <span class="monthWeekday">{item.weekday.trim() || "—"}</span>
          <span class="monthText">{previewText(item.text.trim() || "", preview, 80)}</span>
        </li>
      {:else}
        <li class="empty"><h2>No days indexed.</h2></li>
      {/each}
    </ul>
    <footer class="foot">
      <span>{days.length} days indexed</span>
      <span>Monthly log</span>
    </footer>
  </section>
</article>
