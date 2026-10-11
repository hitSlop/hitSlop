<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { onDestroy, untrack } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { Button } from "bits-ui";
  import doc from "./schema";
  import Chamber, { type Particle } from "./Chamber.svelte";
  import { intakeView, logIntake, resetDay, setTarget } from "./intake";

  const QUICK = [
    { amount: 200, label: "Small glass" },
    { amount: 250, label: "Glass" },
    { amount: 500, label: "Bottle" },
    { amount: 750, label: "Flask" },
  ] as const;
  const PARTICLES = ["💧", "✨", "🎉", "🌟", "🫧", "🌊", "💎", "💙"];
  const view = $derived(intakeView(doc.current));
  const percent = $derived(view.percent);
  const fill = $derived(view.fill);
  const liquid = new Tween(untrack(() => fill), { duration: 350, easing: cubicOut });
  let initialized = false;
  let particles = $state<Particle[]>([]);
  let particleId = 0;
  let audio: AudioContext | undefined;
  let audioTimeout: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    const instant = !initialized || prefersReducedMotion.current;
    void liquid.set(fill, { duration: instant ? 0 : 350, delay: 0 });
    initialized = true;
  });

  $effect(() => {
    if (prefersReducedMotion.current || percent < 100) {
      particles = [];
      stopChime();
    }
  });

  onDestroy(() => {
    void liquid.set(liquid.target, { duration: 0, delay: 0 });
    stopChime();
  });

  function celebrate() {
    if (prefersReducedMotion.current) {
      particles = [];
      return;
    }
    particles = Array.from({ length: 24 }, (_, id) => ({
      id: particleId++,
      symbol: PARTICLES[id % PARTICLES.length]!,
      left: 6 + ((id * 17) % 88),
      size: 16 + (id % 12),
      delay: (id % 12) / 10,
      duration: 2 + (id % 15) / 10,
    }));
    chime();
  }

  function stopChime() {
    clearTimeout(audioTimeout);
    audioTimeout = undefined;
    const ctx = audio;
    audio = undefined;
    if (ctx && ctx.state !== "closed") void ctx.close().catch(() => undefined);
  }

  function chime() {
    stopChime();
    try {
      const Ctx = window.AudioContext || (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (!Ctx) return;
      const ctx = new Ctx();
      audio = ctx;
      audioTimeout = setTimeout(stopChime, 1500);
      [523.25, 659.25, 783.99, 1046.5].forEach((freq, index) => {
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.type = "sine";
        osc.frequency.setValueAtTime(freq, ctx.currentTime + index * 0.09);
        gain.gain.setValueAtTime(0, ctx.currentTime + index * 0.09);
        gain.gain.linearRampToValueAtTime(0.18, ctx.currentTime + index * 0.09 + 0.02);
        gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + index * 0.09 + 0.55);
        osc.connect(gain);
        gain.connect(ctx.destination);
        osc.start(ctx.currentTime + index * 0.09);
        osc.stop(ctx.currentTime + index * 0.09 + 0.6);
        if (index === 3) osc.onended = () => { if (audio === ctx) stopChime(); };
      });
    } catch {
      stopChime();
    }
  }

  function log(amount: number, label: string) {
    const wasComplete = percent >= 100;
    doc.change((tx) => logIntake(tx, doc.current, amount, label, new Date()));
    if (!wasComplete && intakeView(doc.current).complete) celebrate();
  }

  function reset() {
    doc.change((tx) => resetDay(tx, doc.current));
    particles = [];
    stopChime();
  }

  function changeTarget(value: string) {
    const next = Number(value);
    if (Number.isFinite(next) && next > 0) doc.change((tx) => setTarget(tx, next));
  }
</script>



  <main class="flask" aria-label="Water tracker">
    <header class="header">
      <span class="brand">Hydration 01</span>
      <label class="target">
        <span>Target</span>
        <input type="number" min="1" aria-label="Daily target intake" value={doc.current.target} oninput={(event) => changeTarget(event.currentTarget.value)} />
        <span>{doc.current.unit}</span>
      </label>
    </header>

    <Chamber level={liquid.current} {percent} interactive {particles} oncelebrate={celebrate} onparticleend={(id) => { particles = particles.filter((item) => item.id !== id); }} />

    <section class="taps" data-slop-export="hide" aria-label="Quick log drinks">
      {#each QUICK as drink}
        <Button.Root type="button" class="tap" onclick={() => log(drink.amount, drink.label)}>
          <strong>+{drink.amount}</strong>
          <span>{drink.label}</span>
        </Button.Root>
      {/each}
    </section>

    <footer class="foot">
      <span>{view.count ? `${view.count} drinks logged today` : "No drinks logged yet."}</span>
      <Button.Root type="button" data-slop-export="hide" onclick={reset} aria-label="Reset today's water intake">Reset</Button.Root>
    </footer>
  </main>
