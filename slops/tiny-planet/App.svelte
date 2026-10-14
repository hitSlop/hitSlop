<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { bindText, getDocument } from "hitslop/svelte";
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
  import schema, { seasons, type Kind, type Season } from "./schema";
  import { PlanetScene, readPalette, type Tool } from "./planet";
  import { newWorld, placeProblem, placeThing, removeThing, setSeason, worldView } from "./world";

  const doc = getDocument(schema);

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
  const view = $derived(worldView(doc.current));

  $effect(() => { if (notice) { const timer = setTimeout(() => (notice = ""), 3000); return () => clearTimeout(timer); } });
  $effect(() => { scene?.setProps(doc.current.props.map((prop) => ({ id: prop.$id, kind: prop.kind, lat: prop.lat, lon: prop.lon }))); });
  $effect(() => { scene?.setSeed(doc.current.seed); });
  $effect(() => { scene?.setSeason(doc.current.season); });
  $effect(() => { scene?.setTool(tool); });

  async function reroll() {
    // The roll is stored as the seed, so the world it picked is the world that reopens.
    await doc.change((tx) => newWorld(tx, doc.current, Math.random));
    notice = "A brand new world. Your things moved with it.";
  }

  function place(kind: Kind, lat: number, lon: number) {
    const problem = placeProblem(doc.current, lat, lon);
    if (problem) { notice = problem; return; }
    void doc.change((tx) => placeThing(tx, doc.current, kind, lat, lon));
  }

  onMount(() => {
    let created: PlanetScene | undefined;
    let alive = true;
    const motion = matchMedia("(prefers-reduced-motion: reduce)");
    const onMotion = () => created?.setReducedMotion(motion.matches);
    const theme = new MutationObserver(() => created?.setPalette(readPalette(shell)));
    void tick().then(() => {
      if (!alive) return;
      created = new PlanetScene({
        host,
        palette: readPalette(shell),
        seed: doc.current.seed,
        season: doc.current.season,
        reducedMotion: motion.matches,
        onPlace: place,
        onRemove: (id) => void doc.change((tx) => removeThing(tx, id)),
        onMessage: (text) => (notice = text),
      });
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

<main bind:this={shell} class="planet-shell">
  <div bind:this={host} class="planet" role="img" aria-label={`${doc.current.name || "A tiny planet"}: a low-poly world with ${view.things} on it. Drag to spin it.`}></div>

  <header class="planet-head">
    <input aria-label="Planet name" maxlength="32" placeholder="Name your planet" use:bindText={doc.fields.name} />
    <p>{view.things} · {view.season}</p>
  </header>

  <p class="planet-notice" role="status" data-show={!!notice}>{notice || (view.full ? "The planet is full." : tool === "remove" ? "Tap a thing to remove it." : "Tap the land to place. Drag to spin.")}</p>

  <div class="planet-bar" role="toolbar" aria-label="Tools">
    {#each tools as option (option.id)}
      <button class="chip" aria-pressed={tool === option.id} aria-label={option.label} onclick={() => (tool = option.id)}>
        <option.icon size={20} aria-hidden="true" /><span>{option.label}</span>
      </button>
    {/each}
  </div>
  <div class="planet-bar planet-bar-top" role="toolbar" aria-label="Season and world">
    {#each seasons as season (season)}
      {@const Icon = seasonIcons[season]}
      <button class="chip chip-icon" aria-pressed={doc.current.season === season} aria-label={`${season} season`} onclick={() => doc.change((tx) => setSeason(tx, season))}><Icon size={20} aria-hidden="true" /></button>
    {/each}
    <button class="chip chip-icon" aria-label="New world" onclick={reroll}><Dices size={20} aria-hidden="true" /></button>
  </div>
</main>
