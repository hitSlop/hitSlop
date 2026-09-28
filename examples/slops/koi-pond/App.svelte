<script lang="ts">
  import { Slop, useDocument, bindText } from "@hitslop/document/svelte";
  import { capture } from "@hitslop/document/capture";
  import { onMount, tick } from "svelte";
  import { Popover } from "bits-ui";
  import Fish from "@lucide/svelte/icons/fish";
  import Paintbrush from "@lucide/svelte/icons/paintbrush";
  import List from "@lucide/svelte/icons/list";
  import X from "@lucide/svelte/icons/x";
  import schema, { koiPatterns, maxKoi } from "./schema";
  import { PondScene, type Palette } from "./pond";

  const doc = useDocument(schema);
  const names = ["Mochi", "Pip", "Bean", "Noodle", "Sesame", "Dumpling", "Yuzu", "Tofu", "Biscuit", "Plum", "Nori", "Peach", "Button", "Miso", "Taro", "Sprout", "Pebble", "Kiki"];
  const patternLabels: Record<(typeof koiPatterns)[number], string> = { kohaku: "Kohaku", showa: "Showa", ogon: "Ogon", asagi: "Asagi", tancho: "Tancho" };
  const tokens = ["water", "deep", "shallow", "stone", "moss", "lily", "lotus", "paper", "ink", "cream", "red", "gold", "charcoal", "slate", "orange"] as const;

  let shell: HTMLElement;
  let host: HTMLDivElement;
  let scene = $state.raw<PondScene>();
  let listOpen = $state(false);
  let notice = $state("");
  let snapshot = $state("");
  let painting = $state(false);
  const feedings = $derived(doc.current.feedings);
  const feedingLabel = $derived(feedings === null ? "Feeding count unavailable" : `Fed ${feedings} ${feedings === 1 ? "time" : "times"}`);
  const full = $derived(doc.current.koi.length >= maxKoi);
  $effect(() => { if (notice) { const timer = setTimeout(() => notice = "", 3200); return () => clearTimeout(timer); } });
  $effect(() => { scene?.setKoi(doc.current.koi.map(k => ({ id: k.$id, name: k.name, pattern: k.pattern, size: k.size }))); });
  $effect(() => { scene?.setSeed(doc.current.seed); });

  function palette(): Palette {
    const style = getComputedStyle(shell);
    return Object.fromEntries(tokens.map(token => [token, style.getPropertyValue(`--slop-${token}`).trim() || "#888888"])) as Palette;
  }
  function list(items: string[]) {
    return items.length < 3 ? items.join(" & ") : `${items.slice(0, -1).join(", ")} & ${items.at(-1)}`;
  }
  function fed(name?: string) {
    if (feedings === null) { notice = "Feeding count unavailable."; return; }
    doc.fields.feedings.increment();
    notice = name ? `${name} gobbled it up.` : "Snack time!";
  }
  function feed() {
    if (feedings === null) { notice = "Feeding count unavailable."; return; }
    if (!scene || !doc.current.koi.length) { notice = "Add a koi first."; return; }
    fed(scene.feed());
  }
  function addKoi() {
    if (full) { notice = "The pond is full. Nine is plenty!"; return; }
    const taken = new Set(doc.current.koi.map(k => k.name));
    const name = names.find(n => !taken.has(n)) ?? `Koi ${doc.current.koi.length + 1}`;
    const pattern = koiPatterns[Math.floor(Math.random() * koiPatterns.length)]!;
    const size = Math.round((0.8 + Math.random() * 0.4) * 100) / 100;
    doc.fields.koi.insert({ name, pattern, size });
    notice = `Say hello to ${name}.`;
  }
  function release(id: string, name: string) {
    doc.fields.koi.remove(id);
    notice = `${name.trim() || "A koi"} swam off.`;
  }
  function repaint() {
    if (painting) return;
    let seed = doc.current.seed;
    while (seed === doc.current.seed) seed = Math.floor(Math.random() * 1_000_000);
    doc.fields.seed.set(seed);
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
    else if (key === "k") { event.preventDefault(); addKoi(); }
    else if (key === "r") { event.preventDefault(); repaint(); }
  }

  onMount(() => {
    let created: PondScene | undefined;
    let alive = true;
    const motion = matchMedia("(prefers-reduced-motion: reduce)");
    const onMotion = () => created?.setReducedMotion(motion.matches);
    const theme = new MutationObserver(() => created?.setPalette(palette()));
    // Slop mounts its child snippet inside a boundary; wait for element bindings.
    void tick().then(() => {
      if (!alive) return;
      created = new PondScene({ host, palette: palette(), seed: doc.current.seed, reducedMotion: motion.matches, onFeed: fed, onPaintingChange: paintingChanged });
      scene = created;
      motion.addEventListener("change", onMotion);
      theme.observe(document.documentElement, { attributes: true, attributeFilter: ["style", "class"] });
    });
    const stopCapture = capture.onPrepare(async (mode) => {
      if (mode !== "icon" && created) snapshot = await created.snapshot();
    });
    return () => {
      alive = false;
      stopCapture();
      theme.disconnect();
      motion.removeEventListener("change", onMotion);
      created?.destroy();
    };
  });
</script>

<svelte:window onkeydown={shortcut} />

<Slop>
  <main bind:this={shell} class="koi-shell" data-slop-selection="none">
    <div bind:this={host} class="koi-pond" data-empty={!doc.current.koi.length}></div>
    <p class="koi-sr-only">A watercolor koi pond with {doc.current.koi.length} koi{doc.current.koi.length ? `: ${list(doc.current.koi.map(k => k.name.trim() || "unnamed"))}` : ""}. Click the water or press F to feed them.</p>
    {#if !doc.current.koi.length}<p class="koi-empty" aria-hidden="true">A quiet pond.<br /><span>Add a koi to keep it company.</span></p>{/if}

    <div class="koi-badge" aria-hidden="true">Koi Pond · {feedingLabel}</div>

    <div class="koi-controls" role="toolbar" aria-label="Pond">
      <button class="koi-button" onclick={feed} aria-keyshortcuts="F" disabled={!doc.current.koi.length || feedings === null}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="8" cy="14" r="2.6" /><circle cx="15" cy="10" r="2.6" /><circle cx="15.5" cy="17" r="2.2" /></svg>
        <span>Feed</span>
      </button>
      <button class="koi-button" onclick={addKoi} aria-keyshortcuts="K" disabled={full}><Fish size={19} aria-hidden="true" /><span>Add koi</span></button>
      <Popover.Root bind:open={listOpen}>
        <Popover.Trigger class="koi-button koi-icon-button" aria-label="Your koi"><List size={19} aria-hidden="true" /></Popover.Trigger>
        <Popover.Portal>
          <Popover.Content class="koi-popover" side="top" sideOffset={10} align="center" collisionPadding={48}>
            <h2>Your koi</h2>
            {#if doc.current.koi.length}
              <ul class="koi-list">
                {#each doc.current.koi as koi (koi.$id)}
                  <li>
                    <span class="koi-swatch" data-pattern={koi.pattern} title={patternLabels[koi.pattern]}></span>
                    <input aria-label={`Name for ${patternLabels[koi.pattern]} koi`} maxlength="24" use:bindText={doc.at(koi).name} />
                    <button class="koi-release" aria-label={`Release ${koi.name.trim() || "this koi"}`} onclick={() => release(koi.$id, koi.name)}><X size={16} /></button>
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="koi-note">No koi yet.</p>
            {/if}
            <p class="koi-note">{feedingLabel} · F feed · K add · R repaint</p>
          </Popover.Content>
        </Popover.Portal>
      </Popover.Root>
      <button class="koi-button koi-icon-button" onclick={repaint} aria-label={painting ? "Painting pond" : "Repaint pond"} aria-keyshortcuts="R" aria-busy={painting} disabled={painting}><Paintbrush size={18} aria-hidden="true" /></button>
    </div>

    <span class="koi-sr-only" role="status">{notice}</span>
    {#if notice}<div class="koi-notice" aria-hidden="true">{notice}</div>{/if}
  </main>

  {#snippet exportView()}
    <figure class="koi-export">
      {#if snapshot}<img src={snapshot} alt={`Watercolor koi pond with ${list(doc.current.koi.map(k => k.name.trim() || "unnamed"))}`} />{/if}
      <figcaption><strong>Koi Pond</strong><span>{doc.current.koi.length ? list(doc.current.koi.map(k => k.name.trim() || "unnamed")) : "A quiet pond"} · {feedingLabel}</span></figcaption>
    </figure>
  {/snippet}
  {#snippet icon()}
    <svg class="koi-icon" viewBox="0 0 400 400" aria-hidden="true">
      <circle cx="200" cy="200" r="182" fill="var(--slop-stone)" />
      <circle cx="200" cy="200" r="158" fill="var(--slop-water)" />
      <circle cx="188" cy="212" r="104" fill="var(--slop-deep)" opacity=".45" />
      <g transform="rotate(-28 200 200)">
        <path d="M112 150c30-26 86-24 118 0-32 22-88 26-118 0Z" fill="var(--slop-cream)" />
        <path d="M228 150l34-22c-4 14-4 30 0 44Z" fill="var(--slop-cream)" opacity=".8" />
        <circle cx="138" cy="148" r="15" fill="var(--slop-red)" /><circle cx="180" cy="152" r="12" fill="var(--slop-red)" />
        <circle cx="120" cy="142" r="3.6" fill="#1d1d22" /><circle cx="120" cy="158" r="3.6" fill="#1d1d22" />
      </g>
      <g transform="rotate(152 200 200)">
        <path d="M112 150c30-26 86-24 118 0-32 22-88 26-118 0Z" fill="var(--slop-gold)" />
        <path d="M228 150l34-22c-4 14-4 30 0 44Z" fill="var(--slop-gold)" opacity=".8" />
        <circle cx="120" cy="142" r="3.6" fill="#1d1d22" /><circle cx="120" cy="158" r="3.6" fill="#1d1d22" />
      </g>
      <path d="M262 246a38 38 0 1 0 30 40l-30-8Z" fill="var(--slop-lily)" />
      <circle cx="258" cy="270" r="10" fill="var(--slop-lotus)" />
    </svg>
  {/snippet}
</Slop>
