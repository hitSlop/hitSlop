<script lang="ts">
  import { onMount } from "svelte";
  import { faces, journal, kinds, log, startFriends, type Kind } from "./journal.svelte";

  // The iPhone copy of Little Days. Same shared journal as the Mac window: tap here, it lands there.
  const shown = $derived(journal.entries.slice(0, 3));
  onMount(startFriends);
</script>

<div class="phone">
  <div class="status" aria-hidden="true"><b>9:41</b><i class="island"></i><span class="signal">●●● ▮</span></div>

  <header>
    <span class="back" aria-hidden="true">‹</span>
    <h3>Little Days <span aria-hidden="true">♥</span></h3>
    <span class="who" aria-label="Three people editing"><img src={faces.Dad} alt="" width="32" height="32" /><img src={faces.Mia} alt="" width="32" height="32" /><img src={faces.Grandma} alt="" width="32" height="32" /></span>
  </header>
  <p class="live"><i></i>3 editing now</p>

  <div class="quick" role="group" aria-label="Log care">
    {#each Object.entries(kinds) as [kind, meta] (kind)}
      <button type="button" style="--t:{meta.tone}" onclick={() => log(kind as Kind, "Dad")}><span aria-hidden="true">{meta.icon}</span>{meta.label}</button>
    {/each}
  </div>

  <h4>Today, together</h4>
  {#if journal.toast}
    <p class="toast" role="status"><img src={faces[journal.toast.who]} alt="" width="30" height="30" /><span><b>{journal.toast.who}</b> added a {journal.toast.label}</span></p>
  {/if}
  <ol aria-label="Today">
    {#each shown as entry (entry.id)}
      <li style="--t:{kinds[entry.kind].tone}">
        <span class="dot" aria-hidden="true">{kinds[entry.kind].icon}</span>
        <div><b>{kinds[entry.kind].label}</b><small>{entry.text}</small></div>
        <span class="meta"><img src={faces[entry.who]} alt={entry.who} width="30" height="30" />{entry.time}</span>
      </li>
    {/each}
  </ol>
  <div class="bar" aria-hidden="true"><span>Something to remember?</span><i>+</i></div>
</div>

<style>
  .phone { position: relative; width: 390px; height: 844px; padding: 0 22px; display: flex; flex-direction: column; overflow: hidden; color: #29263f; background: #fffaf0; font-family: "Pop Sans", system-ui, sans-serif; text-align: left; }
  .status { height: 62px; padding: 0 12px 0 20px; display: flex; align-items: center; justify-content: space-between; font-size: 17px; }
  .island { width: 118px; height: 34px; border-radius: 20px; background: #0b0918; }
  .signal { color: #29263f; font-size: 11px; letter-spacing: 1px; }
  header { margin-top: 6px; display: flex; align-items: center; gap: 10px; }
  .back { color: #7a7189; font-size: 40px; line-height: .8; }
  h3 { flex: 1; margin: 0; font-family: "Pop Display", sans-serif; font-size: 32px; font-weight: 800; line-height: 1; }
  h3 span { color: #ff5c7f; font-size: 24px; }
  .who { display: flex; }
  .who img { width: 36px; height: 36px; margin-left: -10px; border: 2.5px solid #fffaf0; border-radius: 50%; object-fit: cover; }
  .live { margin: 10px 0 0; display: inline-flex; align-items: center; gap: 8px; color: #1c6b3c; font-size: 15px; font-weight: 800; }
  .live i { width: 9px; height: 9px; border-radius: 50%; background: #25b05a; animation: ping 1.8s ease-in-out infinite; }
  @keyframes ping { 50% { opacity: .35; } }
  .quick { margin-top: 18px; display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
  .quick button { padding: 18px 8px; display: flex; align-items: center; justify-content: center; gap: 10px; border: 2px solid #29263f14; border-radius: 20px; color: #29263f; background: var(--t); font: 800 20px "Pop Sans", sans-serif; cursor: pointer; transition: transform 200ms cubic-bezier(.34, 1.56, .64, 1); }
  .quick button span { font-size: 26px; }
  .quick button:active { transform: scale(.95); }
  h4 { margin: 26px 0 12px; font-family: "Pop Display", sans-serif; font-size: 24px; font-weight: 800; }
  ol { margin: 0; padding: 0; display: grid; gap: 12px; list-style: none; }
  li { padding: 14px; display: flex; align-items: center; gap: 14px; border: 2px solid #f0e4cf; border-radius: 20px; background: #fff; animation: land 360ms cubic-bezier(.34, 1.56, .64, 1); }
  @keyframes land { from { opacity: 0; transform: translateY(-14px) scale(.96); } }
  .dot { width: 50px; height: 50px; flex: none; display: grid; place-items: center; border-radius: 16px; background: var(--t); font-size: 24px; }
  li div { flex: 1; min-width: 0; }
  li b { display: block; font-size: 20px; }
  li small { display: block; overflow: hidden; color: #7a7189; font-size: 16px; text-overflow: ellipsis; white-space: nowrap; }
  .meta { display: grid; justify-items: end; gap: 3px; color: #7a7189; font-size: 14px; font-weight: 700; }
  .meta img { width: 30px; height: 30px; border-radius: 50%; }
  .toast { position: absolute; left: 22px; right: 22px; top: 394px; z-index: 3; margin: 0; padding: 7px 16px 7px 7px; display: flex; align-items: center; gap: 10px; border-radius: 999px; background: #fff; box-shadow: 0 10px 24px #3a241435; font-size: 17px; animation: land 360ms cubic-bezier(.34, 1.56, .64, 1); }
  .toast img { width: 30px; height: 30px; border-radius: 50%; }
  .bar { position: absolute; left: 18px; right: 18px; bottom: 34px; height: 56px; padding: 0 8px 0 20px; display: flex; align-items: center; justify-content: space-between; border-radius: 30px; color: #9a92a8; background: #fff; box-shadow: 0 6px 18px #3a241418; font-size: 17px; }
  .bar i { width: 42px; height: 42px; display: grid; place-items: center; border-radius: 50%; color: #fff; background: #f0127e; font-size: 28px; font-style: normal; }
  @media (prefers-reduced-motion: reduce) { li, .toast, .live i { animation: none; } .quick button { transition: none; } }
</style>
