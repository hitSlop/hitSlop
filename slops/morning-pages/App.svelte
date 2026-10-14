<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { bindText, getDocument } from "hitslop/svelte";
  import { onDestroy, untrack } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { Progress } from "bits-ui";
  import schema from "./schema";
  import { addDays, countWords, dateKey, formatDisplayDate, markFinished, openPage, pageView, TARGET } from "./model";

  const doc = getDocument(schema);
  const { active, wordsCount, progressPct, page1Done, page2Done, page3Done } = $derived(pageView(doc.current));
  const isToday = $derived(doc.current.currentKey === dateKey(new Date()));

  const odometer = new Tween(untrack(() => wordsCount), { duration: 350, easing: cubicOut });
  let initialized = false;
  $effect(() => {
    const instant = !initialized || prefersReducedMotion.current;
    void odometer.set(wordsCount, { duration: instant ? 0 : 350, delay: 0 });
    initialized = true;
  });

  onDestroy(() => { void odometer.set(odometer.target, { duration: 0, delay: 0 }); });

  function openDay(key: string) {
    void doc.change((tx) => openPage(tx, doc.current, key));
  }
  function shiftDate(deltaDays: number) {
    openDay(addDays(doc.current.currentKey, deltaDays));
  }
  // The text itself is written by bindText; this only stamps the page the first time it
  // reaches the target, and only issues a write when it does.
  function stampIfFinished(event: Event) {
    const page = doc.current.entries[doc.current.currentKey];
    const words = countWords((event.currentTarget as HTMLTextAreaElement).value);
    if (!page || page.completedAt || words < TARGET) return;
    void doc.change((tx) => markFinished(tx, page, words, new Date()));
  }
  function sizeToText(node: HTMLTextAreaElement, _value: string) {
    let timer: ReturnType<typeof setTimeout>;
    const resize = () => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        node.style.height = "auto";
        const min = node.parentElement?.clientHeight ?? 0;
        node.style.height = `${Math.max(node.scrollHeight, min)}px`;
      });
    };
    const observer = new ResizeObserver(resize);
    observer.observe(node);
    if (node.parentElement) observer.observe(node.parentElement);
    node.addEventListener("input", resize);
    resize();
    return {
      update: resize,
      destroy() {
        clearTimeout(timer);
        observer.disconnect();
        node.removeEventListener("input", resize);
      },
    };
  }

  const odometerDigits = $derived(String(Math.max(0, Math.round(odometer.current))).padStart(3, "0"));
  const fillScale = $derived(Math.max(0, Math.min(1, odometer.current / TARGET)));
</script>


<main class={"mp-pad"} aria-label="Morning pages legal pad">
  <header class={"mp-stub"}>
    <span class={"mp-perforations"} aria-hidden="true"></span>
    <div class={"mp-brandRow"}>
      <div class={"mp-brand"}>
        <span class={"mp-brandTitle"}>Morning Pages</span>
        <span class={"mp-brandSub"}>Julia Cameron · three handwritten pages</span>
      </div>
      <div class={"mp-dateNav"} data-slop-export="hide">
        <button type="button" class={"mp-navBtn"} onclick={() => shiftDate(-1)} aria-label="Previous day">‹</button>
        <button type="button" class={"mp-navBtn"} data-today="true" onclick={() => openDay(dateKey(new Date()))} disabled={isToday}>Today</button>
        <button type="button" class={"mp-navBtn"} onclick={() => shiftDate(1)} aria-label="Next day">›</button>
      </div>
    </div>
    <div class={"mp-headlineRow"}>
      <h1 class={"mp-dateHeadline"}>{formatDisplayDate(doc.current.currentKey)}</h1>
      {#if page3Done}
        <span class={"mp-stamp"}>3 pages cleared</span>
      {/if}
    </div>
    <Progress.Root
      class={"mp-odometer"}
      value={wordsCount}
      max={TARGET}
      aria-label="Morning Pages word progress"
    >
      <div class={"mp-odometerReadout"}>
        <span class={"mp-odometerDigits"} data-complete={page3Done} aria-hidden="true">{odometerDigits}</span>
        <span class={"mp-odometerPrecise"} aria-live="polite">{wordsCount} / {TARGET} words · {progressPct}%</span>
      </div>
      <div class={"mp-pagesTrack"}>
        <div class={"mp-fillTrack"} aria-hidden="true">
          <div class={"mp-fill"} data-complete={page3Done} style:transform={`scaleX(${fillScale})`}></div>
        </div>
        <div class={"mp-segments"} aria-hidden="true">
          <span class={"mp-segment"} data-done={page1Done}>Page 1 · 250</span>
          <span class={"mp-segment"} data-done={page2Done}>Page 2 · 500</span>
          <span class={"mp-segment"} data-done={page3Done}>Page 3 · 750</span>
        </div>
      </div>
    </Progress.Root>
  </header>

  <article class={"mp-sheet"}>
    <span class={"mp-marginRule"} aria-hidden="true"></span>
    <div class={"mp-holes"} aria-hidden="true">
      <span class={"mp-hole"}></span>
      <span class={"mp-hole"}></span>
      <span class={"mp-hole"}></span>
      <span class={"mp-hole"}></span>
    </div>
    {#if active}
      <textarea
        class={"mp-editor"}
        use:sizeToText={active.text}
        placeholder="Start writing without stopping or self-editing. Pour every thought onto the page until the odometer turns."
        use:bindText={doc.at(active).text}
        oninput={stampIfFinished}
        aria-label="Morning Pages writing area"
        spellcheck="true"
      ></textarea>
    {/if}
  </article>

  <footer class={"mp-footer"} data-slop-export="hide">
    There is no wrong way to do Morning Pages. They are not high art. They are simply moving the hand across the page and writing down whatever comes.
  </footer>

</main>
