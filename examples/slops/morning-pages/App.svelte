<script lang="ts">
  import { bindText, useDocument } from "@hitslop/document/svelte";
  import { capture } from "@hitslop/document/capture";
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { Progress } from "bits-ui";
  import schema from "./schema";
  import { formatDisplayDate, pageView, TARGET } from "./model";

  function dateKey(d: Date): string {
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }
  function todayKey(): string {
    return dateKey(new Date());
  }

  const doc = useDocument(schema);
  // Writes are asynchronous, so an effect can rerun before its write is accepted: each
  // page is created (and marked complete) at most once per session.
  const created = new Set<string>();
  const completed = new Set<string>();
  $effect(() => {
    const key = doc.current.currentKey;
    if (doc.current.entries[key] || created.has(key)) return;
    created.add(key);
    doc.fields.entries.put(key, { date: key, text: "", completedAt: "" }).catch(() => created.delete(key));
  });

  const { active, wordsCount, progressPct, page1Done, page2Done, page3Done } = $derived(pageView(doc.current));
  const isToday = $derived(doc.current.currentKey === todayKey());

  const odometer = new Tween(untrack(() => wordsCount), { duration: 350, easing: cubicOut });
  let initialized = false;
  $effect(() => {
    const instant = !initialized || prefersReducedMotion.current;
    void odometer.set(wordsCount, { duration: instant ? 0 : 350, delay: 0 });
    initialized = true;
  });
  $effect(() => {
    const page = active;
    if (!page || wordsCount < TARGET || page.completedAt || completed.has(page.date)) return;
    completed.add(page.date);
    doc.at(page).completedAt.set(new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }))
      .catch(() => completed.delete(page.date));
  });

  onMount(() => capture.onPrepare(async () => {
    await odometer.set(wordsCount, { duration: 0, delay: 0 });
    await tick();
  }));
  onDestroy(() => { void odometer.set(odometer.target, { duration: 0, delay: 0 }); });

  function openDay(key: string) {
    const create = !doc.current.entries[key] && !created.has(key);
    if (create) created.add(key);
    doc.change((tx) => {
      if (create) tx.fields.entries.put(key, { date: key, text: "", completedAt: "" });
      tx.fields.currentKey.set(key);
    }).catch(() => { if (create) created.delete(key); });
  }
  function shiftDate(deltaDays: number) {
    const [y, m, d] = doc.current.currentKey.split("-").map(Number);
    const date = new Date(y, (m ?? 1) - 1, d);
    date.setDate(date.getDate() + deltaDays);
    openDay(dateKey(date));
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
        <button type="button" class={"mp-navBtn"} data-today="true" onclick={() => openDay(todayKey())} disabled={isToday}>Today</button>
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
        aria-label="Morning Pages writing area"
        spellcheck="true"
      ></textarea>
    {/if}
  </article>

  <footer class={"mp-footer"} data-slop-export="hide">
    There is no wrong way to do Morning Pages. They are not high art. They are simply moving the hand across the page and writing down whatever comes.
  </footer>

</main>
