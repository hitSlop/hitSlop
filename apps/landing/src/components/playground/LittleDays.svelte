<script lang="ts">
  import { onMount } from "svelte";
  import { faces, journal, kinds, log, startFriends, type Kind, type Person } from "./journal.svelte";

  const shown = $derived(journal.entries.slice(0, 4));
  onMount(startFriends);
</script>

<div class="journal">
  <header>
    <span class="sun" aria-hidden="true">
      <svg viewBox="0 0 64 64"><g stroke="#f0c04a" stroke-width="5" stroke-linecap="round"><path d="M32 4v6M32 54v6M4 32h6M54 32h6M12 12l4 4M48 48l4 4M52 12l-4 4M16 48l-4 4" /></g><circle cx="32" cy="32" r="14" fill="#ffd966" /><path d="M26 32v1M38 32v1" stroke="#29263f" stroke-width="3" stroke-linecap="round" /><path d="M26 37q6 6 12 0" fill="none" stroke="#29263f" stroke-width="2.6" stroke-linecap="round" /></svg>
    </span>
    <div class="titles">
      <small>Little Days · a baby journal</small>
      <h3>Maya’s little days</h3>
      <p>Today · 4 months old</p>
    </div>
    <div class="live" aria-label="Three people editing">
      <span class="pill"><i></i>3 editing</span>
      <span class="who"><img src={faces.Dad} alt="" width="44" height="44" /><img src={faces.Mia} alt="" width="44" height="44" /><img src={faces.Grandma} alt="" width="44" height="44" /></span>
    </div>
  </header>

  <div class="body">
    <aside aria-label="Recent care">
      <h4>Recent care</h4>
      <div class="stat" style="--t:#ffe1cf"><span>🍼</span><div><b>Last feed</b><small>2h 10m ago · Dad</small></div></div>
      <div class="stat" style="--t:#d9ebff"><span>💧</span><div><b>Last diaper</b><small>2h 45m ago · Mia</small></div></div>
      <div class="stat" style="--t:#fff0b3"><span>⭐</span><div><b>Latest memory</b><small>First belly laugh</small></div></div>
    </aside>

    <section class="main">
      <div class="quick" role="group" aria-label="Log care">
        {#each Object.entries(kinds) as [kind, meta] (kind)}
          <button type="button" style="--t:{meta.tone}" onclick={() => log(kind as Kind, "Dad")}><span aria-hidden="true">{meta.icon}</span>{meta.label}</button>
        {/each}
      </div>
      {#if journal.toast}
        <p class="toast" role="status"><img src={faces[journal.toast.who]} alt="" width="26" height="26" /><span><b>{journal.toast.who}</b> added a {journal.toast.label} <em>just now</em></span></p>
      {/if}
      <ol class="timeline" aria-label="Today">
        {#each shown as entry (entry.id)}
          <li style="--t:{kinds[entry.kind].tone}">
            <span class="dot" aria-hidden="true">{kinds[entry.kind].icon}</span>
            <div><b>{kinds[entry.kind].label}</b><small>{entry.text}</small></div>
            <span class="meta"><em>{entry.who}</em>{entry.time}</span>
          </li>
        {/each}
      </ol>
    </section>
  </div>
  <p class="foot">Little moments. Lovingly kept. <span aria-hidden="true">♡</span></p>
</div>

<style>
  .journal { position: relative; width: 900px; height: 604px; padding: 26px 38px 18px; display: flex; flex-direction: column; gap: 14px; overflow: hidden; color: #29263f; background: #fffaf0; font-family: "Pop Sans", system-ui, sans-serif; text-align: left; }
  header { display: flex; align-items: center; gap: 22px; padding-bottom: 14px; border-bottom: 2px solid #f0e4cf; }
  .sun { width: 74px; height: 74px; flex: none; display: grid; place-items: center; border: 2px solid #f0e4cf; border-radius: 50%; background: radial-gradient(circle at 35% 30%, #fff2d6, #ffd2bd); }
  .sun svg { width: 58px; }
  .titles { flex: 1; min-width: 0; }
  small { color: #7a7189; }
  .titles small { font-size: 13px; font-weight: 800; letter-spacing: .16em; text-transform: uppercase; }
  h3 { margin: 4px 0 2px; font-family: "Pop Display", sans-serif; font-size: 40px; font-weight: 800; line-height: 1; letter-spacing: -.01em; }
  .titles p { margin: 0; color: #7a7189; font-size: 19px; }
  .live { display: grid; justify-items: end; gap: 8px; }
  .pill { padding: 3px 12px 3px 9px; display: inline-flex; align-items: center; gap: 7px; border-radius: 999px; color: #1c6b3c; background: #dff5e6; font-size: 14px; font-weight: 800; }
  .pill i { width: 9px; height: 9px; border-radius: 50%; background: #25b05a; animation: ping 1.8s ease-in-out infinite; }
  @keyframes ping { 50% { opacity: .35; } }
  .who { display: flex; }
  .who img { width: 46px; height: 46px; margin-left: -12px; border: 3px solid #fffaf0; border-radius: 50%; object-fit: cover; }
  .toast { position: absolute; right: 38px; top: 150px; z-index: 3; margin: 0; padding: 6px 14px 6px 6px; display: flex; align-items: center; gap: 9px; border-radius: 999px; background: #fff; box-shadow: 0 8px 22px #3a241430; font-size: 15px; animation: land 360ms cubic-bezier(.34, 1.56, .64, 1); }
  .toast img { width: 26px; height: 26px; border-radius: 50%; }
  .toast em { color: #e0527f; font-style: normal; font-weight: 800; }
  .body { flex: 1; min-height: 0; display: grid; grid-template-columns: 260px 1fr; gap: 28px; }
  aside h4 { margin: 0 0 12px; color: #7a7189; font-size: 13px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
  .stat { margin-bottom: 12px; padding: 14px; display: flex; align-items: center; gap: 12px; border-radius: 18px; background: var(--t); }
  .stat span { font-size: 28px; }
  .stat b { display: block; font-size: 17px; }
  .stat small { font-size: 14px; }
  .main { min-width: 0; display: flex; flex-direction: column; gap: 16px; }
  .quick { display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; }
  .quick button { padding: 14px 8px; display: grid; justify-items: center; gap: 4px; border: 2px solid #29263f14; border-radius: 18px; color: #29263f; background: var(--t); font: 800 17px "Pop Sans", sans-serif; cursor: pointer; transition: transform 200ms cubic-bezier(.34, 1.56, .64, 1), box-shadow 200ms; }
  .quick button span { font-size: 28px; }
  .quick button:hover { transform: translateY(-3px) rotate(-1.5deg); box-shadow: 0 8px 0 #29263f0f; }
  .quick button:active { transform: translateY(1px) scale(.97); }
  .timeline { margin: 0; padding: 0; display: grid; gap: 8px; list-style: none; }
  .timeline li { padding: 8px 16px; display: flex; align-items: center; gap: 14px; border: 2px solid #f0e4cf; border-radius: 16px; background: #fff; animation: land 360ms cubic-bezier(.34, 1.56, .64, 1); }
  @keyframes land { from { opacity: 0; transform: translateY(-14px) scale(.96); } }
  .dot { width: 44px; height: 44px; flex: none; display: grid; place-items: center; border-radius: 14px; background: var(--t); font-size: 22px; }
  .timeline div { flex: 1; min-width: 0; }
  .timeline b { display: block; font-size: 17px; }
  .timeline small { display: block; overflow: hidden; font-size: 15px; text-overflow: ellipsis; white-space: nowrap; }
  .meta { display: grid; justify-items: end; color: #7a7189; font-size: 14px; font-weight: 700; }
  .meta em { padding: 1px 10px; border-radius: 999px; color: #29263f; background: #f3ecdf; font-style: normal; font-size: 13px; }
  .foot { margin: 0; color: #7a7189; font-size: 14px; }
  .foot span { color: #e07d94; }
  @media (prefers-reduced-motion: reduce) { .timeline li, .toast, .pill i { animation: none; } .quick button { transition: none; } }
</style>
