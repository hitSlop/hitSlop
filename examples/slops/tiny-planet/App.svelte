<script lang="ts">
  import { bindText, capture } from "@hitslop/document/svelte";
  import { onMount, tick } from "svelte";
  import TreePine from "@lucide/svelte/icons/tree-pine";
  import House from "@lucide/svelte/icons/house";
  import Mountain from "@lucide/svelte/icons/mountain";
  import Gem from "@lucide/svelte/icons/gem";
  import Eraser from "@lucide/svelte/icons/eraser";
  import Dices from "@lucide/svelte/icons/dices";
  import Flower from "@lucide/svelte/icons/flower-2";
  import Sun from "@lucide/svelte/icons/sun";
  import Leaf from "@lucide/svelte/icons/leaf";
  import Snowflake from "@lucide/svelte/icons/snowflake";
  import doc, { maxProps, seasons, type Season } from "./schema";
  import { PlanetScene, type Palette, type Tool } from "./planet";
  import { ui } from "./ui.svelte";

  const tokens = ["ocean", "sand", "grass", "forest", "rock", "snow", "autumn", "blossom", "wall", "roof", "trunk", "crystal"] as const;
  const tools: { id: Tool; label: string; icon: typeof TreePine }[] = [
    { id: "tree", label: "Tree", icon: TreePine },
    { id: "house", label: "House", icon: House },
    { id: "rock", label: "Rock", icon: Mountain },
    { id: "crystal", label: "Crystal", icon: Gem },
    { id: "remove", label: "Remove", icon: Eraser },
  ];
  const seasonIcons: Record<Season, typeof Sun> = { spring: Flower, summer: Sun, autumn: Leaf, winter: Snowflake };

  let shell: HTMLElement;
  let host: HTMLDivElement;
  let scene = $state.raw<PlanetScene>();
  let tool = $state<Tool>("tree");
  let notice = $state("");
  const count = $derived(doc.current.props.length);
  const full = $derived(count >= maxProps);

  $effect(() => { if (notice) { const timer = setTimeout(() => (notice = ""), 3000); return () => clearTimeout(timer); } });
  $effect(() => { scene?.setProps(doc.current.props.map((prop) => ({ id: prop.$id, kind: prop.kind, lat: prop.lat, lon: prop.lon }))); });
  $effect(() => { scene?.setSeed(doc.current.seed); });
  $effect(() => { scene?.setSeason(doc.current.season); });
  $effect(() => { scene?.setTool(tool); });

  function palette(): Palette {
    const style = getComputedStyle(shell);
    return Object.fromEntries(tokens.map((token) => [token, style.getPropertyValue(`--slop-${token}`).trim() || "#888888"])) as unknown as Palette;
  }

  function reroll() {
    let seed = doc.current.seed;
    while (seed === doc.current.seed) seed = Math.floor(Math.random() * 1_000_000);
    doc.fields.seed.set(seed);
    notice = "A brand new world. Your things moved with it.";
  }

  onMount(() => {
    let created: PlanetScene | undefined;
    let alive = true;
    const motion = matchMedia("(prefers-reduced-motion: reduce)");
    const onMotion = () => created?.setReducedMotion(motion.matches);
    const theme = new MutationObserver(() => created?.setPalette(palette()));
    void tick().then(() => {
      if (!alive) return;
      created = new PlanetScene({
        host,
        palette: palette(),
        seed: doc.current.seed,
        season: doc.current.season,
        reducedMotion: motion.matches,
        onPlace: (kind, lat, lon) => {
          if (doc.current.props.length >= maxProps) { notice = "The planet is full. Remove something first."; return; }
          doc.fields.props.insert({ kind, lat, lon });
        },
        onRemove: (id) => doc.fields.props.remove(id),
        onMessage: (text) => (notice = text),
      });
      scene = created;
      motion.addEventListener("change", onMotion);
      theme.observe(document.documentElement, { attributes: true, attributeFilter: ["style", "class"] });
    });
    const stopCapture = capture.onPrepare(async (mode) => {
      if (mode !== "icon" && created) ui.snapshot = created.snapshot();
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

<main bind:this={shell} class="planet-shell" data-slop-selection="none">
  <div bind:this={host} class="planet" role="img" aria-label={`${doc.current.name || "A tiny planet"}: a low-poly world with ${count} ${count === 1 ? "thing" : "things"} on it. Drag to spin it.`}></div>

  <header class="planet-head">
    <input aria-label="Planet name" maxlength="32" placeholder="Name your planet" use:bindText={doc.fields.name} />
    <p>{count} {count === 1 ? "thing" : "things"} · {doc.current.season}</p>
  </header>

  <p class="planet-notice" role="status" data-show={!!notice}>{notice || (full ? "The planet is full." : tool === "remove" ? "Tap a thing to remove it." : "Tap the land to place. Drag to spin.")}</p>

  <div class="planet-bar" role="toolbar" aria-label="Tools">
    {#each tools as option}
      <button class="chip" aria-pressed={tool === option.id} aria-label={option.label} onclick={() => (tool = option.id)}>
        <option.icon size={20} aria-hidden="true" /><span>{option.label}</span>
      </button>
    {/each}
  </div>
  <div class="planet-bar planet-bar-top" role="toolbar" aria-label="Season and world">
    {#each seasons as season}
      {@const Icon = seasonIcons[season]}
      <button class="chip chip-icon" aria-pressed={doc.current.season === season} aria-label={`${season} season`} onclick={() => doc.fields.season.set(season)}><Icon size={20} aria-hidden="true" /></button>
    {/each}
    <button class="chip chip-icon" aria-label="New world" onclick={reroll}><Dices size={20} aria-hidden="true" /></button>
  </div>
</main>
