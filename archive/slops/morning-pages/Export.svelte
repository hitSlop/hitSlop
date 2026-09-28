<script lang="ts">
  
  const TARGET = 750;
  let { dateLabel, text, words, completed }: {
    dateLabel: string;
    text: string;
    words: number;
    completed: boolean;
  } = $props();
  const progressPct = $derived(Math.min(100, Math.round((words / TARGET) * 100)));
  const fillScale = $derived(Math.max(0, Math.min(1, words / TARGET)));
  const page1Done = $derived(words >= 250);
  const page2Done = $derived(words >= 500);
</script>

<article class={"mp-exportPad"} aria-label="Exported morning pages for {dateLabel}">
  <header class={"mp-stub"}>
    <span class={"mp-perforations"} aria-hidden="true"></span>
    <div class={"mp-brandRow"}>
      <div class={"mp-brand"}>
        <span class={"mp-brandTitle"}>Morning Pages</span>
        <span class={"mp-brandSub"}>Julia Cameron · three handwritten pages</span>
      </div>
    </div>
    <div class={"mp-headlineRow"}>
      <h1 class={"mp-dateHeadline"}>{dateLabel}</h1>
      {#if completed}<span class={"mp-stamp"}>3 pages cleared</span>{/if}
    </div>
    <div class={"mp-odometer"}>
      <div class={"mp-odometerReadout"}>
        <span class={"mp-odometerDigits"} data-complete={completed}>{String(words).padStart(3, "0")}</span>
        <span class={"mp-odometerPrecise"}>{words} / {TARGET} words · {progressPct}%</span>
      </div>
      <div class={"mp-pagesTrack"}>
        <div class={"mp-fillTrack"} aria-hidden="true">
          <div class={"mp-fill"} data-complete={completed} style:transform={`scaleX(${fillScale})`}></div>
        </div>
        <div class={"mp-segments"} aria-hidden="true">
          <span class={"mp-segment"} data-done={page1Done}>Page 1 · 250</span>
          <span class={"mp-segment"} data-done={page2Done}>Page 2 · 500</span>
          <span class={"mp-segment"} data-done={completed}>Page 3 · 750</span>
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
