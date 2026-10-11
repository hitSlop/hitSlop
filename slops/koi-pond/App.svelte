<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { bindText } from "hitslop/svelte";
  import { onMount, tick } from "svelte";
  import { Popover } from "bits-ui";
  import Fish from "@lucide/svelte/icons/fish";
  import Paintbrush from "@lucide/svelte/icons/paintbrush";
  import List from "@lucide/svelte/icons/list";
  import X from "@lucide/svelte/icons/x";
  import doc from "./schema";
  import { addKoi as collectKoi, feedKoi, maxName, patternLabels, pondView, releaseKoi, repaintPond } from "./shared";
  import { PondScene, readPalette } from "./pond";

  let shell: HTMLElement;
  let host: HTMLDivElement;
  let scene = $state.raw<PondScene>();
  let listOpen = $state(false);
  let notice = $state("");
  let painting = $state(false);
  const view = $derived(pondView(doc.current));
  $effect(() => { if (notice) { const timer = setTimeout(() => notice = "", 3200); return () => clearTimeout(timer); } });
  // The scene is a view of the document: it rebuilds its fish and painting from saved data.
  $effect(() => { scene?.setKoi(doc.current.koi.map(k => ({ id: k.$id, name: k.name, pattern: k.pattern, size: k.size }))); });
  $effect(() => { scene?.setSeed(doc.current.seed); });

  async function fed(name?: string) {
    await doc.change(tx => feedKoi(tx, doc.current));
    notice = name ? `${name} gobbled it up.` : "Snack time!";
  }
  function feed() {
    if (!scene || !doc.current.koi.length) { notice = "Add a koi first."; return; }
    void fed(scene.feed());
  }
  async function addKoi() {
    if (view.full) { notice = "The pond is full. Nine is plenty!"; return; }
    const { id } = await doc.change(tx => collectKoi(tx, doc.current, Math.random));
    const name = doc.current.koi.find(k => k.$id === id)?.name;
    notice = `Say hello to ${name || "your new koi"}.`;
  }
  async function release(id: string) {
    const name = await doc.change(tx => releaseKoi(tx, doc.current, id));
    notice = `${name.trim() || "A koi"} swam off.`;
  }
  function repaint() {
    if (painting) return;
    void doc.change(tx => repaintPond(tx, doc.current, Math.random));
  }
  function paintingChanged(busy: boolean) {
    if (busy === painting) return;
    painting = busy;
    notice = busy ? "Painting a fresh pond…" : "A fresh coat of watercolor.";
  }
  function shortcut(event: KeyboardEvent) {
    if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey || listOpen || event.target instanceof HTMLInputElement) return;
    const key = event.key.toLowerCase();
    if (key === "f") { event.preventDefault(); feed(); }
    else if (key === "k") { event.preventDefault(); void addKoi(); }
    else if (key === "r") { event.preventDefault(); repaint(); }
  }

  onMount(() => {
    let created: PondScene | undefined;
    let alive = true;
    const motion = matchMedia("(prefers-reduced-motion: reduce)");
    const onMotion = () => created?.setReducedMotion(motion.matches);
    const theme = new MutationObserver(() => created?.setPalette(readPalette(shell)));
    // Wait for element bindings before building the scene.
    void tick().then(() => {
      if (!alive) return;
      created = new PondScene({ host, palette: readPalette(shell), seed: doc.current.seed, reducedMotion: motion.matches, random: Math.random, onFeed: (name) => void fed(name), onPaintingChange: paintingChanged });
      scene = created;
      motion.addEventListener("change", onMotion);
      theme.observe(document.documentElement, { attributes: true, attributeFilter: ["style", "class"] });
    });
    return () => {
      alive = false;
      theme.disconnect();
      motion.removeEventListener("change", onMotion);
      created?.destroy();
    };
  });
</script>

<svelte:window onkeydown={shortcut} />

<main bind:this={shell} class="koi-shell">
  <div bind:this={host} class="koi-pond" data-empty={!view.count}></div>
  <p class="koi-sr-only">A watercolor koi pond with {view.count} koi{view.count ? `: ${view.names}` : ""}. Click the water or press F to feed them.</p>
  {#if !view.count}<p class="koi-empty" aria-hidden="true">A quiet pond.<br /><span>Add a koi to keep it company.</span></p>{/if}

  <div class="koi-badge" aria-hidden="true">Koi Pond · {view.feedingLabel}</div>

  <div class="koi-controls" role="toolbar" aria-label="Pond">
    <button class="koi-button" onclick={feed} aria-keyshortcuts="F" disabled={!view.count}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="8" cy="14" r="2.6" /><circle cx="15" cy="10" r="2.6" /><circle cx="15.5" cy="17" r="2.2" /></svg>
      <span>Feed</span>
    </button>
    <button class="koi-button" onclick={addKoi} aria-keyshortcuts="K" disabled={view.full}><Fish size={19} aria-hidden="true" /><span>Add koi</span></button>
    <Popover.Root bind:open={listOpen}>
      <Popover.Trigger class="koi-button koi-icon-button" aria-label="Your koi"><List size={19} aria-hidden="true" /></Popover.Trigger>
      <Popover.Portal>
        <Popover.Content class="koi-popover" side="top" sideOffset={10} align="center" collisionPadding={48}>
          <h2>Your koi</h2>
          {#if view.count}
            <ul class="koi-list">
              {#each doc.current.koi as koi (koi.$id)}
                <li>
                  <span class="koi-swatch" data-pattern={koi.pattern} title={patternLabels[koi.pattern]}></span>
                  <input aria-label={`Name for ${patternLabels[koi.pattern]} koi`} maxlength={maxName} use:bindText={doc.at(koi).name} />
                  <button class="koi-release" aria-label={`Release ${koi.name.trim() || "this koi"}`} onclick={() => release(koi.$id)}><X size={16} /></button>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="koi-note">No koi yet.</p>
          {/if}
          <p class="koi-note">{view.feedingLabel} · F feed · K add · R repaint</p>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
    <button class="koi-button koi-icon-button" onclick={repaint} aria-label={painting ? "Painting pond" : "Repaint pond"} aria-keyshortcuts="R" aria-busy={painting} disabled={painting}><Paintbrush size={18} aria-hidden="true" /></button>
  </div>

  <span class="koi-sr-only" role="status">{notice}</span>
  {#if notice}<div class="koi-notice" aria-hidden="true">{notice}</div>{/if}
</main>
