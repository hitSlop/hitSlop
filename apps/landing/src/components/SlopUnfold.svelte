<script lang="ts">
  import { onMount } from "svelte";

  // The real SQLite tables (docs/reference/runtime.md): the app's, then your document's.
  const app = [
    { name: "app", note: "its name, shape and starting values", emoji: "🏷️" },
    { name: "assets", note: "the little app itself", emoji: "🧩" },
    { name: "artwork", note: "Quick Look preview and icon", emoji: "👀" },
  ];
  const yours = [
    { name: "checkpoint + updates", note: "everything you’ve saved", emoji: "💾" },
    { name: "document", note: "its own identity, your colors too", emoji: "🎨" },
    { name: "attachments", note: "photos and files you add", emoji: "📎" },
  ];
  // null = server/no-JS: everything visible. After mount it folds, then opens on first view.
  let open = $state<boolean | null>(null);
  let hover = $state(false);
  let root: HTMLElement;

  // A tiny ASCII file with feelings. Padding is computed so the box always lines up.
  const W = 16;
  const row = (text = "") => {
    const left = Math.floor((W - text.length) / 2);
    return "|" + " ".repeat(left) + text + " ".repeat(W - left - text.length) + "|";
  };
  const face = $derived(open === false ? (hover ? "(o w o)" : "(u w u)") : "\\(O w O)/");
  const art = $derived([
    "." + "-".repeat(W) + ".",
    row("~ SQLite ~"),
    row(),
    row(face),
    row(open === false ? "<3" : "!!"),
    row(),
    "'" + "-".repeat(W) + "'",
    "    |" + " ".repeat(W - 8) + "|",
    "   _|" + " ".repeat(W - 8) + "|_",
  ].join("\n"));

  onMount(() => {
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) { open = true; return; }
    open = false;
    const observer = new IntersectionObserver(([entry]) => {
      if (entry?.isIntersecting) { setTimeout(() => (open = true), 350); observer.disconnect(); }
    }, { threshold: .5 });
    observer.observe(root);
    return () => observer.disconnect();
  });
</script>

<div class="unfold" data-open={open === null ? "static" : String(open)} bind:this={root}>
  <button type="button" class="doc" onclick={() => (open = !open)} onpointerenter={() => (hover = true)} onpointerleave={() => (hover = false)} aria-expanded={open !== false} aria-controls="slop-parts">
    <span class="bubble" aria-hidden="true">{open === false ? "i’m literally just a file ✨" : "ok ok, those are my insides >_<"}</span>
    <pre class="ascii" aria-hidden="true">{art}</pre>
    <span class="doc-name">weekend-trip.slop</span>
    <span class="doc-action">{open === false ? "Peek inside" : "Fold it back up"}</span>
  </button>
  <div class="parts" id="slop-parts">
    <section class="group">
      <h3>The app's tables <small>stay exactly as built</small></h3>
      <ul>{#each app as part, i}<li style:--i={i}><span aria-hidden="true">{part.emoji}</span><code>{part.name}</code><small>{part.note}</small></li>{/each}</ul>
    </section>
    <section class="group yours">
      <h3>Your tables <small>change as you use it</small></h3>
      <ul>{#each yours as part, i}<li style:--i={i + app.length}><span aria-hidden="true">{part.emoji}</span><code>{part.name}</code><small>{part.note}</small></li>{/each}</ul>
    </section>
  </div>
</div>

<style>
  .unfold { display: grid; gap: 28px; align-items: center; }
  @media (min-width: 760px) { .unfold { grid-template-columns: auto minmax(0, 1fr); } }
  .doc { justify-self: center; width: 190px; padding: 0; display: grid; justify-items: center; gap: 10px; border: 0; background: none; color: var(--ink); font: inherit; cursor: pointer; }
  .doc { width: 230px; }
  .bubble { position: relative; max-width: 220px; padding: 10px 14px; border: 2.5px solid #10132c; border-radius: 18px 18px 18px 4px; background: #fff; box-shadow: 0 4px 0 #10132c; font-family: "HitSlop Handwriting", cursive; font-size: 1.2rem; line-height: 1.1; rotate: -3deg; }
  .bubble::after { position: absolute; left: 22px; bottom: -12px; width: 16px; height: 12px; border-left: 2.5px solid #10132c; background: #fff; transform: skewX(-30deg); content: ""; }
  .ascii { margin: 4px 0 0; padding: 12px 16px; border-radius: 16px; color: var(--accent); background: #fff; box-shadow: 0 5px 0 #10132c1a, 0 18px 30px #2a1b5c1f; font-family: "SFMono-Regular", "Cascadia Code", ui-monospace, monospace; font-size: 15px; font-weight: 700; line-height: 1.15; text-align: left; transition: transform 500ms var(--spring), color 400ms; }
  .doc:hover .ascii { transform: rotate(-3deg) translateY(-4px); }
  .unfold[data-open="true"] .ascii { transform: rotate(3deg) scale(1.02); }
  @media (prefers-reduced-motion: no-preference) { .unfold[data-open="false"] .ascii { animation: bob 2.6s ease-in-out infinite; } }
  @keyframes bob { 50% { translate: 0 -5px; } }
  .doc-name { font-family: "SFMono-Regular", ui-monospace, monospace; font-size: .95rem; font-weight: 700; }
  .doc-action { padding: 6px 14px; border: 2px solid #10132c; border-radius: 99px; background: #ffe66b; box-shadow: 0 3px 0 #10132c; font-size: .9rem; font-weight: 800; }
  .parts { display: grid; gap: 16px; }
  @media (min-width: 1100px) { .parts { grid-template-columns: 1.3fr 1fr; align-items: start; } }
  .group { padding: 18px; border: 2px solid #10132c; border-radius: 20px; background: #fff; box-shadow: 0 5px 0 #10132c; }
  .group.yours { background: #fff7d6; }
  h3 { margin-bottom: 10px; display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 10px; font-family: "HitSlop Display", sans-serif; font-size: 1.35rem; font-weight: 800; }
  h3 small { color: var(--muted); font-family: "HitSlop Sans", sans-serif; font-size: .9rem; font-weight: 600; }
  ul { margin: 0; padding: 0; display: grid; gap: 6px; list-style: none; }
  li { min-height: 44px; padding: 6px 10px; display: grid; grid-template-columns: 30px minmax(0, 1fr); column-gap: 8px; align-items: center; border-radius: 12px; background: #f7f5fb; transition: transform 520ms var(--spring), opacity 300ms; transition-delay: calc(var(--i) * 45ms); }
  .yours li { background: #fffdf2; }
  li span { grid-row: span 2; font-size: 1.3rem; }
  li code { font-size: .9rem; font-weight: 700; overflow-wrap: anywhere; }
  li small { color: var(--muted); font-size: .85rem; }
  .unfold[data-open="false"] li { opacity: 0; transform: translateX(-60px) scale(.6) rotate(-6deg); }
  @media (prefers-reduced-motion: reduce) { .unfold[data-open="false"] li { opacity: 1; transform: none; } li, .ascii { transition: none; } }
</style>
