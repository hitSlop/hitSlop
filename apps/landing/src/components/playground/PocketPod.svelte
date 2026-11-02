<script lang="ts">
  import { onMount } from "svelte";

  // A playable stand-in for Pocket Pod: the wheel's arrows spin Cover Flow, play/pause runs a progress bar.
  const tracks = [
    { title: "Neon Bus Ride", artist: "The Lo-Fi Club", a: "#ff5cae", b: "#ffd23f" },
    { title: "Sunday Sunroom", artist: "Pocket Pals", a: "#3aa0ff", b: "#9be7ff" },
    { title: "Little Fireworks", artist: "Mia & Leo", a: "#8a5cff", b: "#ff8ad0" },
    { title: "Tomato Timer Jam", artist: "Deep Work Radio", a: "#ff5a4a", b: "#ffb26b" },
    { title: "Slushie Weather", artist: "Cup Holders", a: "#19c6a7", b: "#b9f2a1" },
  ];
  let index = $state(0);
  let playing = $state(false);
  let seconds = $state(0);
  const track = $derived(tracks[index]!);
  const clock = $derived(`${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`);

  function move(step: number): void {
    index = (index + step + tracks.length) % tracks.length;
    seconds = 0;
  }
  function toggle(): void { playing = !playing; }

  onMount(() => {
    const timer = setInterval(() => { if (playing) seconds = (seconds + 1) % 200; }, 1000);
    return () => clearInterval(timer);
  });
</script>

<div class="pod">
  <span class="sticker star" aria-hidden="true"><svg viewBox="0 0 40 40"><path d="m20 3 5 11 12 2-9 8 3 12-11-6-11 6 3-12-9-8 12-2Z" fill="#ffd23f" stroke="#e0a400" stroke-width="2.4" stroke-linejoin="round" /></svg></span>
  <span class="sticker smile" aria-hidden="true"><svg viewBox="0 0 40 40"><circle cx="20" cy="20" r="17" fill="#ffd23f" stroke="#e0a400" stroke-width="3" /><path d="M13 16v3M27 16v3" stroke="#3b2a00" stroke-width="3" stroke-linecap="round" /><path d="M12 25q8 8 16 0" fill="none" stroke="#3b2a00" stroke-width="3" stroke-linecap="round" /></svg></span>
  <span class="sticker heart" aria-hidden="true"><svg viewBox="0 0 40 40"><path d="M20 35S4 25 4 14a8 8 0 0 1 16-3 8 8 0 0 1 16 3c0 11-16 21-16 21Z" fill="#ff5cae" stroke="#fff" stroke-width="3" stroke-linejoin="round" /></svg></span>

  <div class="screen">
    <div class="status"><b>{playing ? "Now Playing" : "Cover Flow"}</b><span class="battery" aria-hidden="true"></span></div>
    <div class="flow" aria-live="polite">
      {#each tracks as t, i (t.title)}
        {@const d = ((i - index + tracks.length + 2) % tracks.length) - 2}
        <button type="button" class="cover" tabindex="-1" aria-label={t.title} style="--d:{d}; --a:{t.a}; --b:{t.b}" onclick={() => move(d)}><i></i></button>
      {/each}
    </div>
    <div class="meta"><b>{track.title}</b><span>{track.artist}</span></div>
    {#if playing}
      <div class="bar" aria-hidden="true"><span style="width:{(seconds / 200) * 100}%"></span><small>{clock}</small></div>
    {/if}
  </div>

  <p class="brand" aria-hidden="true">POCKET POD</p>

  <div class="wheel">
    <button type="button" class="menu" onclick={() => { playing = false; }} aria-label="Menu">MENU</button>
    <button type="button" class="prev" onclick={() => move(-1)} aria-label="Previous track"><svg viewBox="0 0 24 16"><path d="M12 8 21 1v14ZM2 8l9-7v14Z" /></svg></button>
    <button type="button" class="next" onclick={() => move(1)} aria-label="Next track"><svg viewBox="0 0 24 16"><path d="M12 8 3 1v14Zm10 0-9-7v14Z" /></svg></button>
    <button type="button" class="play" onclick={toggle} aria-label={playing ? "Pause" : "Play"} aria-pressed={playing}><span aria-hidden="true">▶︎❚❚</span></button>
    <button type="button" class="center" onclick={toggle} aria-label="Select"></button>
  </div>
</div>

<style>
  .pod { position: relative; width: 360px; height: 603px; padding: 38px 24px 0; border-radius: 40px; background: repeating-linear-gradient(90deg, #ffffff10 0 1px, transparent 1px 3px), linear-gradient(180deg, #e9ebf0, #c4c8d2 55%, #b0b4c0); border: 1px solid #ffffffb0; box-shadow: inset 0 2px 0 #fff, inset 0 -3px 0 #8f94a2, 0 0 0 1px #5a5f7255; font-family: "Pop Sans", system-ui, sans-serif; user-select: none; }
  .screen { height: 296px; padding: 0 0 12px; display: flex; flex-direction: column; overflow: hidden; border: 5px solid #1a1a22; border-radius: 16px; background: linear-gradient(#fff, #eef0f5); box-shadow: inset 0 0 0 1px #0004; }
  .status { height: 28px; padding: 0 12px; display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid #8e97aa; background: linear-gradient(#f4f6fa, #b5bdcc); font-size: 15px; color: #111; }
  .battery { width: 22px; height: 10px; border-radius: 2px; background: #111; }
  .flow { position: relative; flex: 1; perspective: 520px; }
  .cover { position: absolute; left: 50%; top: 20px; width: 124px; height: 124px; padding: 0; border: 0; cursor: pointer; background: linear-gradient(135deg, var(--a), var(--b)); border-radius: 4px; box-shadow: 0 10px 14px #0003; transform: translateX(calc(-50% + var(--d) * 58px)) translateZ(calc(var(--d) * var(--d) * -30px)) rotateY(calc(var(--d) * -52deg)); opacity: calc(1 - var(--d) * var(--d) * .22); z-index: calc(10 - var(--d) * var(--d)); transition: transform 460ms cubic-bezier(.34, 1.4, .5, 1), opacity 300ms; }
  .cover i { position: absolute; inset: 0; border-radius: inherit; background: radial-gradient(circle at 30% 28%, #fff8, transparent 45%), repeating-linear-gradient(45deg, #fff2 0 6px, transparent 6px 14px); }
  .meta { padding: 0 12px; display: grid; justify-items: center; gap: 1px; text-align: center; }
  .meta b { max-width: 100%; overflow: hidden; font-size: 17px; text-overflow: ellipsis; white-space: nowrap; }
  .meta span { color: #6b7080; font-size: 14px; }
  .bar { position: relative; height: 10px; margin: 8px 18px 0; border: 1px solid #8e97aa; border-radius: 5px; background: #dfe3ec; }
  .bar span { position: absolute; inset: 0 auto 0 0; border-radius: 4px; background: linear-gradient(#4ea3ff, #1f6fe0); }
  .bar small { position: absolute; right: 0; top: 11px; color: #6b7080; font-size: 11px; }
  .brand { margin: 22px 0 0; color: #9096a6; font-size: 14px; font-weight: 800; letter-spacing: .5em; text-align: center; text-shadow: 0 1px 0 #fff; }
  .wheel { position: absolute; left: 50%; bottom: 36px; width: 236px; height: 236px; border-radius: 50%; background: radial-gradient(circle at 50% 30%, #fff, #f1f2f5 70%, #e4e6ec); box-shadow: 0 2px 6px #0003, inset 0 -2px 4px #0001; transform: translateX(-50%); }
  .wheel button { position: absolute; padding: 0; border: 0; background: none; color: #1c1d26; cursor: pointer; font-family: inherit; }
  .wheel svg { width: 24px; height: 16px; fill: currentColor; }
  .menu { left: 50%; top: 16px; translate: -50% 0; color: #6f7587 !important; font-size: 15px; font-weight: 800; letter-spacing: .08em; }
  .prev { left: 20px; top: 50%; translate: 0 -50%; }
  .next { right: 20px; top: 50%; translate: 0 -50%; }
  .play { left: 50%; bottom: 14px; translate: -50% 0; font-size: 17px; letter-spacing: -.1em; }
  .wheel button:not(.center):hover { color: var(--accent, #f0127e); }
  .wheel button:focus-visible { outline: 3px solid #1f6fe0; outline-offset: 3px; border-radius: 6px; }
  .center { left: 50%; top: 50%; width: 92px; height: 92px; translate: -50% -50%; border-radius: 50% !important; background: radial-gradient(circle at 40% 30%, #fff, #e0e3ea 80%) !important; box-shadow: 0 0 0 1px #0002, inset 0 -3px 5px #0001; }
  .center:active { scale: .96; }
  .sticker { position: absolute; z-index: 3; pointer-events: none; filter: drop-shadow(0 2px 2px #0003); }
  .sticker svg { display: block; width: 100%; height: 100%; }
  .star { left: 8px; top: 346px; width: 54px; height: 54px; rotate: -14deg; }
  .smile { right: 6px; top: 342px; width: 58px; height: 58px; rotate: 10deg; }
  .heart { right: 14px; bottom: 80px; width: 48px; height: 48px; rotate: 14deg; }
  @media (prefers-reduced-motion: reduce) { .cover { transition: none; } }
</style>
