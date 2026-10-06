<script lang="ts">
  
  import { onDestroy, untrack } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { Button } from "bits-ui";
  import doc from "./schema";
  import Chamber, { type Particle } from "./Chamber.svelte";
  import { percentOf } from "./intake";

  const QUICK = [
    { amount: 200, label: "Small glass" },
    { amount: 250, label: "Glass" },
    { amount: 500, label: "Bottle" },
    { amount: 750, label: "Flask" },
  ] as const;
  const PARTICLES = ["💧", "✨", "🎉", "🌟", "🫧", "🌊", "💎", "💙"];
  const percent = $derived(percentOf(doc.current.current, doc.current.target));
  const fill = $derived(Math.max(4, percent));
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

  function logIntake(amount: number, label: string) {
    const now = new Date();
    const wasComplete = percent >= 100;
    const nextCurrent = doc.current.current + amount;
    const nextPercent = percentOf(nextCurrent, doc.current.target);
    const time = `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
    const first = doc.current.logs[0];
    doc.change((tx) => {
      tx.fields.current.set(nextCurrent);
      tx.fields.logs.insert({ time, amount, label }, first ? { before: first.$id } : undefined);
    });
    if (!wasComplete && nextPercent >= 100) celebrate();
  }

  function resetDay() {
    doc.change((tx) => {
      tx.fields.current.set(0);
      for (const log of doc.current.logs) tx.fields.logs.remove(log.$id);
    });
    particles = [];
    stopChime();
  }

  function setTarget(value: string) {
    const next = Number(value);
    if (Number.isFinite(next) && next > 0) doc.fields.target.set(next);
  }
</script>



  <main class="flask" data-slop-selection="none" aria-label="Water tracker">
    <header class="header">
      <span class="brand">Hydration 01</span>
      <label class="target">
        <span>Target</span>
        <input type="number" min="1" aria-label="Daily target intake" value={doc.current.target} oninput={(event) => setTarget(event.currentTarget.value)} />
        <span>{doc.current.unit}</span>
      </label>
    </header>

    <Chamber level={liquid.current} {percent} interactive {particles} oncelebrate={celebrate} onparticleend={(id) => { particles = particles.filter((item) => item.id !== id); }} />

    <section class="taps" data-slop-export="hide" aria-label="Quick log drinks">
      {#each QUICK as drink}
        <Button.Root type="button" class="tap" onclick={() => logIntake(drink.amount, drink.label)}>
          <strong>+{drink.amount}</strong>
          <span>{drink.label}</span>
        </Button.Root>
      {/each}
    </section>

    <footer class="foot">
      <span>{doc.current.logs.length ? `${doc.current.logs.length} drinks logged today` : "No drinks logged yet."}</span>
      <Button.Root type="button" data-slop-export="hide" onclick={resetDay} aria-label="Reset today's water intake">Reset</Button.Root>
    </footer>
  </main>
