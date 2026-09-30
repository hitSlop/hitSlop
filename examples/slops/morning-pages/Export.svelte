<script lang="ts">
import { useDocument } from "@hitslop/document/svelte";
import schema from "./schema";
import { formatDisplayDate, pageView, TARGET } from "./model";
const doc = useDocument(schema);
const { active, wordsCount, progressPct, page1Done, page2Done, page3Done } = $derived(pageView(doc.current));
const text = $derived(active?.text ?? "");
</script>

<article class={"mp-exportPad"} aria-label="Exported morning pages for {formatDisplayDate(doc.current.currentKey)}">
  <header class={"mp-stub"}>
    <span class={"mp-perforations"} aria-hidden="true"></span>
    <div class={"mp-brandRow"}>
      <div class={"mp-brand"}>
        <span class={"mp-brandTitle"}>Morning Pages</span>
        <span class={"mp-brandSub"}>Julia Cameron · three handwritten pages</span>
      </div>
    </div>
    <div class={"mp-headlineRow"}>
      <h1 class={"mp-dateHeadline"}>{formatDisplayDate(doc.current.currentKey)}</h1>
      {#if page3Done}<span class={"mp-stamp"}>3 pages cleared</span>{/if}
    </div>
    <div class={"mp-odometer"}>
      <div class={"mp-odometerReadout"}>
        <span class={"mp-odometerDigits"} data-complete={page3Done}>{String(wordsCount).padStart(3, "0")}</span>
        <span class={"mp-odometerPrecise"}>{wordsCount} / {TARGET} words · {progressPct}%</span>
      </div>
      <div class={"mp-pagesTrack"}>
        <div class={"mp-fillTrack"} aria-hidden="true">
          <div class={"mp-fill"} data-complete={page3Done} style:transform={`scaleX(${Math.min(1, wordsCount / TARGET)})`}></div>
        </div>
        <div class={"mp-segments"} aria-hidden="true">
          <span class={"mp-segment"} data-done={page1Done}>Page 1 · 250</span>
          <span class={"mp-segment"} data-done={page2Done}>Page 2 · 500</span>
          <span class={"mp-segment"} data-done={page3Done}>Page 3 · 750</span>
        </div>
      </div>
    </div>
  </header>
  <section class={"mp-sheet"}>
    <span class={"mp-marginRule"} aria-hidden="true"></span>
    <div class={"mp-holes"} aria-hidden="true">
      <span class={"mp-hole"}></span>
      <span class={"mp-hole"}></span>
      <span class={"mp-hole"}></span>
      <span class={"mp-hole"}></span>
    </div>
    {#if text.trim()}
      <p class={"mp-writing"}>{text}</p>
    {:else}
      <p class={"mp-empty"}>Nothing written this morning.</p>
    {/if}
  </section>
</article>
