<script lang="ts">
  type Template = {
    slug: string; title: string; description: string; categories: string[];
    shape: string; width: number; height: number; emoji: string;
    colors: { background: string; accent: string; ink: string };
    icon?: string; preview?: string;
  };
  let { templates }: { templates: Template[] } = $props();

  const labels: Record<string, string> = {
    productivity: "✅ Get things done", personal: "🌱 Just for you", education: "🎓 School",
    finance: "💰 Money", utilities: "🧰 Handy tools", business: "💼 Work", media: "🎧 Music & media",
    games: "🎮 Games", "developer-tools": "🛠️ For developers", other: "✨ Other",
  };
  const categories = $derived(
    [...new Set(templates.flatMap((template) => template.categories))]
      .filter((category) => labels[category])
      .sort((a, b) => templates.filter((t) => t.categories.includes(b)).length - templates.filter((t) => t.categories.includes(a)).length),
  );
  let filter = $state<string | null>(null);
  let spinning = $state<string | null>(null);
  let chosen = $state<Template | null>(null);
  let dialog: HTMLDialogElement;
  const matching = $derived(filter ? templates.filter((template) => template.categories.includes(filter!)) : templates);
  // A taste first; the whole shelf is one click away. A filter always shows every match.
  const PREVIEW = 18;
  let expanded = $state(false);
  const visible = $derived(expanded || filter ? matching : matching.slice(0, PREVIEW));
  // A stable, hand-made scatter: every tile leans a little, never the same way twice in a row.
  const tilt = (index: number) => [-1.6, 1.2, -.6, 1.8, -1.2, .7][index % 6];

  function show(template: Template): void {
    chosen = template;
    dialog.showModal();
  }
  function surprise(): void {
    if (spinning) return;
    const pool = matching;
    const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const pick = pool[Math.floor(Math.random() * pool.length)]!;
    expanded = true;
    if (reduce) { show(pick); return; }
    let step = 0;
    const steps = 12;
    const hop = () => {
      spinning = step < steps - 1 ? pool[Math.floor(Math.random() * pool.length)]!.slug : pick.slug;
      step += 1;
      if (step < steps) setTimeout(hop, 30 + step * 10);
      else setTimeout(() => { spinning = null; show(pick); }, 360);
    };
    hop();
  }
  const ink = (template: Template) => template.colors.ink;
</script>

<div class="wall">
  <div class="controls">
    <div class="chips" role="group" aria-label="Filter templates">
      <button type="button" aria-pressed={filter === null} onclick={() => (filter = null)}>All <span>{templates.length}</span></button>
      {#each categories as category}
        <button type="button" aria-pressed={filter === category} onclick={() => (filter = filter === category ? null : category)}>{labels[category]}</button>
      {/each}
    </div>
    <button type="button" class="surprise" onclick={surprise} disabled={!!spinning}><span aria-hidden="true">🎲</span> Surprise me</button>
  </div>

  <ul class="tiles" aria-live="polite">
    {#each visible as template, index (template.slug)}
      <li style:--tilt={`${tilt(index)}deg`}>
        <button
          type="button"
          class="tile"
          data-shape={template.shape}
          data-spinning={spinning === template.slug}
          style:--bg={template.colors.background}
          style:--accent={template.colors.accent}
          style:--ink={ink(template)}
          onclick={() => show(template)}
          aria-label={`${template.title}: ${template.description}`}
        >
          <span class="emoji" aria-hidden="true">{#if template.icon}<img src={template.icon} alt="" loading="lazy" />{:else}{template.emoji}{/if}</span>
          <span class="name">{template.title}</span>
          <span class="file" aria-hidden="true">{template.slug}.slop</span>
        </button>
      </li>
    {/each}
  </ul>
  {#if !expanded && !filter && templates.length > PREVIEW}
    <div class="more"><button type="button" onclick={() => (expanded = true)}>See all {templates.length} tiny apps <span aria-hidden="true">↓</span></button></div>
  {/if}
</div>

<dialog class="detail" bind:this={dialog} aria-labelledby="template-detail-title" onclick={(event) => { if (event.target === dialog) dialog.close(); }}>
  {#if chosen}
    <div class="detail-window">
      <div class="detail-bar">
        <form method="dialog"><button class="light" aria-label="Close">×</button></form>
        <span>{chosen.slug}.slop</span>
      </div>
      <div class="detail-art" style:--bg={chosen.colors.background} style:--accent={chosen.colors.accent} style:--ink={chosen.colors.ink}>
        {#if chosen.preview}<img src={chosen.preview} alt={`${chosen.title} preview`} />{:else}<span aria-hidden="true">{chosen.emoji}</span>{/if}
      </div>
      <div class="detail-copy">
        <p class="badge">✓ Included with hitSlop</p>
        <h3 id="template-detail-title">{chosen.title}</h3>
        <p>{chosen.description}</p>
        <p class="meta">{chosen.categories.map((category) => labels[category] ?? category).join("  ·  ")}  ·  {chosen.shape === "ellipse" ? "a round little window" : `${chosen.width} × ${chosen.height}`}</p>
        <p class="hint">Open hitSlop, pick <strong>{chosen.title}</strong> from Templates, and make a copy. It’s yours to keep, change, or send.</p>
      </div>
    </div>
  {/if}
</dialog>

<style>
  .controls { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 14px; }
  .chips { display: flex; flex-wrap: wrap; gap: 8px; }
  .chips button { min-height: 42px; padding: 0 15px; display: inline-flex; align-items: center; gap: 8px; border: 1px solid #e2def1; border-radius: 999px; color: var(--ink); background: #fff; font: inherit; font-size: .95rem; font-weight: 750; cursor: pointer; transition: transform 220ms var(--spring), background-color 160ms; }
  .chips button span { padding: 1px 8px; border-radius: 99px; background: #f1e9ff; font-size: .8rem; }
  .chips button[aria-pressed="true"] { color: #fff; border-color: var(--ink); background: var(--ink); }
  .chips button[aria-pressed="true"] span { background: #ffffff2e; }
  .chips button:hover { transform: translateY(-2px) rotate(-1deg); }
  .surprise { min-height: 48px; padding: 0 20px; display: inline-flex; align-items: center; gap: 8px; border: 1px solid #e6ca45; border-radius: 18px; color: #10132c; background: #ffe66b; box-shadow: inset 0 2px 0 #fff7bc, 0 3px 0 #d9b92e; font: inherit; font-family: "HitSlop Display", sans-serif; font-size: 1.1rem; font-weight: 800; cursor: pointer; transition: transform 220ms var(--spring); }
  .surprise:hover { transform: translateY(-2px) rotate(2deg); }
  .surprise:disabled { cursor: wait; }
  .tiles { margin: 34px 0 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(168px, 1fr)); gap: 18px; list-style: none; }
  li { rotate: var(--tilt); }
  .tile { position: relative; width: 100%; aspect-ratio: 1 / 1.08; padding: 16px 14px 14px; display: grid; grid-template-rows: 1fr auto auto; justify-items: start; gap: 4px; overflow: hidden; border: 2px solid #10132c; border-radius: 22px; color: var(--ink); background: var(--bg); box-shadow: 0 5px 0 #10132c, 0 14px 22px #2a1b5c22; font: inherit; text-align: left; cursor: pointer; transition: transform 300ms var(--spring), box-shadow 300ms var(--spring); }
  .tile::after { position: absolute; right: -18px; top: -18px; width: 70px; height: 70px; border-radius: 50%; background: var(--accent); opacity: .5; content: ""; transition: transform 400ms var(--spring); }
  .tile[data-shape="ellipse"] .emoji { border-radius: 50%; }
  .emoji { position: relative; z-index: 1; width: 64px; height: 64px; display: grid; place-items: center; align-self: center; border: 2px solid #10132c; border-radius: 18px; background: #fff; box-shadow: 3px 4px 0 #10132c33; font-size: 34px; line-height: 1; transition: transform 380ms var(--spring); }
  .emoji img { width: 100%; height: 100%; object-fit: contain; border-radius: inherit; }
  .name { position: relative; z-index: 1; font-family: "HitSlop Display", sans-serif; font-size: 1.15rem; font-weight: 800; line-height: 1.05; }
  .file { position: relative; z-index: 1; max-width: 100%; overflow: hidden; font-family: "SFMono-Regular", ui-monospace, monospace; font-size: .74rem; opacity: .72; text-overflow: ellipsis; white-space: nowrap; }
  .tile:hover, .tile:focus-visible, .tile[data-spinning="true"] { transform: translateY(-6px) rotate(calc(var(--tilt) * -1.5)); box-shadow: 0 11px 0 #10132c, 0 22px 30px #2a1b5c30; }
  .tile:hover .emoji, .tile[data-spinning="true"] .emoji { transform: rotate(-10deg) scale(1.12); }
  .tile:hover::after { transform: scale(1.4); }
  .tile:active { transform: translateY(2px); box-shadow: 0 2px 0 #10132c; }
  .tile[data-spinning="true"] { outline: 4px solid var(--sun); outline-offset: 3px; }
  .more { margin-top: 34px; display: flex; justify-content: center; }
  .more button { min-height: 52px; padding: 0 24px; border: 2px solid #10132c; border-radius: 18px; color: #10132c; background: #fff; box-shadow: 0 4px 0 #10132c; font: inherit; font-family: "HitSlop Display", sans-serif; font-size: 1.15rem; font-weight: 800; cursor: pointer; transition: transform 220ms var(--spring); }
  .more button:hover { transform: translateY(-3px) rotate(-1deg); }
  .more button:active { transform: translateY(2px); box-shadow: 0 1px 0 #10132c; }
  .detail { width: min(760px, calc(100vw - 24px)); padding: 0; border: 0; background: transparent; overflow: visible; }
  .detail::backdrop { background: #10132c73; backdrop-filter: blur(5px); }
  .detail[open] .detail-window { animation: pop 420ms var(--spring); }
  @keyframes pop { from { opacity: 0; transform: scale(.85) rotate(-2deg); } }
  .detail-window { overflow: hidden; border: 2px solid #10132c; border-radius: 22px; background: #fff; box-shadow: 0 8px 0 #10132c, 0 40px 80px #10132c55; }
  .detail-bar { height: 40px; padding: 0 12px; display: flex; align-items: center; gap: 10px; border-bottom: 2px solid #10132c; background: #f7f5fb; font-family: "SFMono-Regular", ui-monospace, monospace; font-size: .85rem; font-weight: 700; }
  .detail-bar form { margin: 0; }
  .detail-bar .light { width: 22px; height: 22px; display: grid; place-items: center; padding: 0; border: 0; border-radius: 50%; color: #7a1c14; background: #ff5f57; font-size: 15px; font-weight: 900; cursor: pointer; }
  .detail-art { min-height: 220px; display: grid; place-items: center; background: radial-gradient(circle at 80% 20%, var(--accent), transparent 45%), var(--bg); font-size: 96px; }
  .detail-art img { max-height: 340px; width: auto; }
  .detail-copy { padding: 22px 26px 26px; }
  .badge { margin: 0 0 8px; display: inline-block; padding: 4px 10px; border-radius: 99px; color: #11573a; background: #c7f3dc; font-size: .85rem; font-weight: 800; }
  .detail-copy h3 { font-family: "HitSlop Headline", "HitSlop Display", sans-serif; font-size: 2.2rem; font-weight: 900; line-height: 1; }
  .detail-copy p { margin: 10px 0 0; color: var(--muted); font-size: 1.05rem; line-height: 1.55; }
  .detail-copy .meta { font-size: .9rem; font-weight: 700; }
  .detail-copy .hint { padding: 12px 14px; border-radius: 14px; background: #f6f2ff; font-size: .95rem; }
  @media (max-width: 560px) {
    .controls { display: grid; }
    .chips { flex-wrap: nowrap; overflow-x: auto; margin-inline: -20px; padding: 4px 20px 8px; scrollbar-width: none; }
    .chips button { flex: none; }
    .surprise { justify-self: start; }
    .tiles { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; }
    .tile { padding: 12px 11px 11px; border-radius: 18px; }
    .emoji { width: 52px; height: 52px; font-size: 28px; border-radius: 15px; }
    .name { font-size: 1rem; }
    .file { font-size: .68rem; }
  }
  @media (prefers-reduced-motion: reduce) { .tile, .emoji, li { transition: none; rotate: 0deg; } }
</style>
