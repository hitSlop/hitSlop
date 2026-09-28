<script lang="ts">
  import { onMount } from "svelte";

  const steps = [
    { note: "Make a project and open it in your coding agent", command: "bunx @hitslop/cli init my-slop" },
    { note: "Try it in the browser while you describe what you want", command: "bun run dev" },
    { note: "Build the finished app", command: "bun run build" },
    { note: "Put it in hitSlop’s template catalog", command: "bun run register" },
  ];
  let typed = $state(steps.map(() => ""));
  let current = $state(-1);
  let done = $state(false);
  let root: HTMLElement;
  let timers: ReturnType<typeof setTimeout>[] = [];

  function finish(): void {
    typed = steps.map((step) => step.command);
    current = steps.length;
    done = true;
  }
  function play(): void {
    timers.forEach(clearTimeout);
    timers = [];
    typed = steps.map(() => "");
    done = false;
    current = 0;
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) { finish(); return; }
    let delay = 300;
    steps.forEach((step, index) => {
      timers.push(setTimeout(() => { current = index; }, delay));
      for (let i = 1; i <= step.command.length; i++) {
        delay += 34 + Math.random() * 40;
        timers.push(setTimeout(() => { typed[index] = step.command.slice(0, i); }, delay));
      }
      delay += 520;
    });
    timers.push(setTimeout(() => { current = steps.length; done = true; }, delay));
  }

  onMount(() => {
    const observer = new IntersectionObserver(([entry]) => {
      if (entry?.isIntersecting) { play(); observer.disconnect(); }
    }, { threshold: .45 });
    observer.observe(root);
    return () => { observer.disconnect(); timers.forEach(clearTimeout); };
  });
</script>

<figure class="terminal" bind:this={root} aria-label="Making a slop in the terminal">
  <div class="bar">
    <span class="lights" aria-hidden="true"><i></i><i></i><i></i></span>
    <span>Terminal — my-slop</span>
    <button type="button" onclick={play} aria-label="Replay the terminal">↻ Replay</button>
  </div>
  <div class="screen">
    {#each steps as step, index}
      <div class="step" class:shown={current === -1 || current >= index}>
        <p class="note"># {step.note}</p>
        <p class="line"><span class="prompt" aria-hidden="true">$</span><code>{current === -1 ? step.command : typed[index]}</code>{#if current === index && typed[index] !== step.command}<span class="caret" aria-hidden="true"></span>{/if}</p>
      </div>
    {/each}
  </div>
  <figcaption class="result" class:done>
    <span class="file" aria-hidden="true"><img src="/assets/appicon-128.webp" width="44" height="44" alt="" /></span>
    <span><code>my-slop.slop</code><small>Your app, ready to open in hitSlop.</small></span>
  </figcaption>
</figure>

<style>
  .terminal { margin: 0; overflow: hidden; border-radius: 18px; color: #e9ecf8; background: #151a2b; box-shadow: 0 30px 60px #10132c3d, 0 6px 0 #0b0e1a; rotate: .8deg; }
  .bar { min-height: 46px; padding: 0 14px; display: flex; align-items: center; gap: 14px; background: #1f2539; color: #aeb5d4; font-size: .85rem; font-weight: 700; }
  .lights { display: flex; gap: 7px; }
  .lights i { width: 12px; height: 12px; border-radius: 50%; background: #ff5f57; }
  .lights i:nth-child(2) { background: #febc2e; }
  .lights i:nth-child(3) { background: #28c840; }
  .bar button { margin-left: auto; padding: 5px 10px; border: 1px solid #ffffff26; border-radius: 10px; color: #e9ecf8; background: transparent; font: inherit; font-size: .8rem; cursor: pointer; }
  .bar button:hover { background: #ffffff14; }
  .screen { min-height: 300px; padding: 22px 24px 8px; font-family: "SFMono-Regular", ui-monospace, monospace; font-size: .95rem; }
  .step { margin-bottom: 18px; opacity: .25; transition: opacity 300ms; }
  .step.shown { opacity: 1; }
  .note { margin: 0 0 6px; color: #8e96bb; font-size: .85rem; }
  .line { margin: 0; display: flex; align-items: center; gap: 10px; min-height: 1.5em; }
  .prompt { color: #69e3a1; }
  .caret { width: 9px; height: 1.15em; background: #69e3a1; animation: blink 900ms steps(1) infinite; }
  @keyframes blink { 50% { opacity: 0; } }
  .result { padding: 16px 22px; display: flex; align-items: center; gap: 14px; background: #173024; }
  .result code { display: block; color: #fff; font-size: 1rem; font-weight: 700; }
  .result small { color: #9fd5b6; font-size: .9rem; }
  .file { width: 52px; height: 52px; display: grid; place-items: center; border-radius: 14px; background: #fff; box-shadow: 0 6px 14px #0006; transform: scale(.4) rotate(-20deg); opacity: .25; transition: transform 600ms var(--spring), opacity 300ms; }
  .file img { border-radius: 10px; }
  .result.done .file { transform: none; opacity: 1; animation: bounce 900ms var(--spring) 1; }
  @keyframes bounce { 30% { transform: translateY(-14px) rotate(8deg) scale(1.1); } 60% { transform: translateY(0) rotate(-4deg); } }
  @media (prefers-reduced-motion: reduce) { .file, .result.done .file { transform: none; opacity: 1; animation: none; } .caret { animation: none; } }
</style>
