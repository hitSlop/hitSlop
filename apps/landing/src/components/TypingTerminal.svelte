<script lang="ts">
  import { onMount } from "svelte";

  // The prompts mirror `slop init` (packages/hitslop/src/cli/init.ts, agents.ts).
  type Step =
    | { kind: "command"; note?: string; text: string }
    | { kind: "prompt"; question: string; text: string }
    | { kind: "choice"; question: string; options: string[] }
    | { kind: "output"; text: string };
  const steps: Step[] = [
    { kind: "command", note: "Make a project", text: "bunx hitslop init my-slop" },
    { kind: "prompt", question: "What should your slop do?", text: "A packing list for short trips. Group items by bag and show how many are left." },
    { kind: "choice", question: "Which agent CLI should build your slop?", options: ["Codex", "Claude Code", "Gemini CLI", "OpenCode"] },
    { kind: "output", text: "Launching Codex in ~/my-slop" },
    { kind: "command", note: "Try it in the browser while you refine it", text: "bun run dev" },
    { kind: "command", note: "Build the finished app", text: "bun run build" },
    { kind: "command", note: "Put it in hitSlop’s template catalog", text: "bun run register" },
  ];
  const typable = (step: Step): string => (step.kind === "command" || step.kind === "prompt" ? step.text : "");
  let typed = $state(steps.map(() => ""));
  let current = $state(-1);
  let done = $state(false);
  let root: HTMLElement;
  let timers: ReturnType<typeof setTimeout>[] = [];

  function finish(): void {
    typed = steps.map(typable);
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
      const text = typable(step);
      for (let i = 1; i <= text.length; i++) {
        delay += 34 + Math.random() * 40;
        timers.push(setTimeout(() => { typed[index] = text.slice(0, i); }, delay));
      }
      delay += text ? 520 : 800;
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
        {#if step.kind === "command" && step.note}<p class="note"># {step.note}</p>{/if}
        {#if step.kind === "command"}
          <p class="line"><span class="prompt" aria-hidden="true">$</span><code>{current === -1 ? step.text : typed[index]}</code>{#if current === index && typed[index] !== step.text}<span class="caret" aria-hidden="true"></span>{/if}</p>
        {:else if step.kind === "prompt"}
          <p class="ask"><span class="prompt" aria-hidden="true">?</span> {step.question}</p>
          <p class="answer"><code>{current === -1 ? step.text : typed[index]}</code>{#if current === index && typed[index] !== step.text}<span class="caret" aria-hidden="true"></span>{/if}</p>
        {:else if step.kind === "choice"}
          <p class="ask"><span class="prompt" aria-hidden="true">?</span> {step.question}</p>
          <ul class="choices">{#each step.options as option, i}<li class:picked={i === 0}><span aria-hidden="true">{i === 0 ? "❯" : " "}</span> {option}</li>{/each}</ul>
        {:else}
          <p class="out">{step.text}</p>
        {/if}
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
  .screen { min-height: 420px; padding: 22px 24px 8px; font-family: "SFMono-Regular", ui-monospace, monospace; font-size: .95rem; }
  .step { margin-bottom: 18px; opacity: .25; transition: opacity 300ms; }
  .step.shown { opacity: 1; }
  .note { margin: 0 0 6px; color: #8e96bb; font-size: .85rem; }
  .line { margin: 0; display: flex; align-items: center; gap: 10px; min-height: 1.5em; }
  .prompt { color: #69e3a1; }
  .ask { margin: 0 0 4px; color: #e9ecf8; font-weight: 700; }
  .answer { margin: 0 0 0 22px; color: #ffe66b; overflow-wrap: anywhere; }
  .choices { margin: 0 0 0 22px; padding: 0; list-style: none; color: #8e96bb; }
  .choices .picked { color: #69e3a1; }
  .out { margin: 0; color: #8e96bb; }
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
