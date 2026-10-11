// A small Web Audio player. The sound is synthesized here (no soundfont, no network):
// a soft triangle-and-sine voice with a piano-like decay, plus a wood-block click.
// The AudioContext is created on the first Play press and closed on dispose.
import type { Plan } from "./score";

const LOOKAHEAD = 0.15;
const TICK_MS = 25;

export type PlayerOptions = {
  click: () => boolean;
  /** Called near the end of each pass: the next pass (the tempo may have changed), or null to finish. */
  next: () => Plan | null;
  onEnd: () => void;
};

export class Player {
  private ctx: AudioContext | null = null;
  private bus: GainNode | null = null;
  private timer: ReturnType<typeof setInterval> | null = null;
  private plan: Plan | null = null;
  private options: PlayerOptions | null = null;
  private passStart = 0;
  private noteIndex = 0;
  private clickIndex = 0;
  private pending: Plan["clicks"] = [];
  private final = false;
  passes = 0;

  /** Must be called from a user gesture: it creates or resumes the audio context. */
  start(plan: Plan, options: PlayerOptions & { countIn: boolean }) {
    this.stop();
    if (!this.ctx) {
      const Context = window.AudioContext ?? (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
      this.ctx = new Context();
    }
    const ctx = this.ctx;
    void ctx.resume();
    const bus = ctx.createGain();
    const tone = ctx.createBiquadFilter();
    tone.type = "lowpass";
    tone.frequency.value = 3200;
    bus.gain.value = 0.9;
    bus.connect(tone).connect(ctx.destination);
    this.bus = bus;
    this.plan = plan;
    this.options = options;
    this.passes = 0;
    this.final = false;
    const lead = options.countIn ? -Math.min(...plan.countIn.map(c => c.t)) : 0;
    this.passStart = ctx.currentTime + 0.08 + lead;
    this.pending = options.countIn ? plan.countIn.slice() : [];
    this.noteIndex = 0;
    this.clickIndex = 0;
    this.timer = setInterval(() => this.tick(), TICK_MS);
    this.tick();
  }

  /** Where playback is: the pass, and seconds into it (negative during the count-in). */
  position(): { plan: Plan; seconds: number } | null {
    if (!this.ctx || !this.plan) return null;
    return { plan: this.plan, seconds: this.ctx.currentTime - this.passStart };
  }

  get playing() { return this.plan !== null; }

  stop() {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    if (this.bus && this.ctx) {
      // Fade the bus rather than cutting it, then let it go.
      const bus = this.bus;
      bus.gain.setTargetAtTime(0, this.ctx.currentTime, 0.015);
      setTimeout(() => bus.disconnect(), 120);
    }
    this.bus = null;
    this.plan = null;
  }

  dispose() {
    this.stop();
    void this.ctx?.close();
    this.ctx = null;
  }

  private tick() {
    const ctx = this.ctx, plan = this.plan, options = this.options;
    if (!ctx || !plan || !options || !this.bus) return;
    const horizon = ctx.currentTime + LOOKAHEAD;
    while (this.pending.length && this.passStart + this.pending[0].t < horizon) {
      const click = this.pending.shift()!;
      this.click(this.passStart + click.t, click.accent, true);
    }
    while (this.noteIndex < plan.notes.length && this.passStart + plan.notes[this.noteIndex].t < horizon) {
      const note = plan.notes[this.noteIndex++];
      this.voice(this.passStart + note.t, note.dur, note.pitch, note.volume);
    }
    while (this.clickIndex < plan.clicks.length && this.passStart + plan.clicks[this.clickIndex].t < horizon) {
      const click = plan.clicks[this.clickIndex++];
      if (options.click()) this.click(this.passStart + click.t, click.accent, false);
    }
    const passEnd = this.passStart + plan.length;
    if (!this.final && horizon >= passEnd) {
      const next = options.next();
      if (next) {
        this.passStart = passEnd;
        this.plan = next;
        this.noteIndex = 0;
        this.clickIndex = 0;
        this.passes++;
      } else this.final = true;
    }
    if (this.final && ctx.currentTime >= passEnd + 0.1) {
      this.stop();
      options.onEnd();
    }
  }

  private voice(at: number, duration: number, pitch: number, volume: number) {
    const ctx = this.ctx!, bus = this.bus!;
    const frequency = 440 * 2 ** ((pitch - 69) / 12);
    const peak = 0.16 * Math.min(1.2, Math.max(0.2, volume / 100));
    const env = ctx.createGain();
    env.gain.setValueAtTime(0, at);
    env.gain.linearRampToValueAtTime(peak, at + 0.008);
    env.gain.setTargetAtTime(peak * 0.3, at + 0.008, 0.35);
    const end = at + duration * 0.95;
    env.gain.setTargetAtTime(0, end, 0.06);
    env.connect(bus);
    const body = ctx.createOscillator();
    body.type = "triangle";
    body.frequency.value = frequency;
    const shine = ctx.createOscillator();
    shine.type = "sine";
    shine.frequency.value = frequency * 2;
    const shineGain = ctx.createGain();
    shineGain.gain.setValueAtTime(0.35, at);
    shineGain.gain.setTargetAtTime(0.05, at, 0.12);
    body.connect(env);
    shine.connect(shineGain).connect(env);
    const stop = end + 0.4;
    body.start(at); shine.start(at);
    body.stop(stop); shine.stop(stop);
    body.onended = () => env.disconnect();
  }

  private click(at: number, accent: boolean, countIn: boolean) {
    const ctx = this.ctx!, bus = this.bus!;
    const osc = ctx.createOscillator();
    const env = ctx.createGain();
    osc.type = "square";
    osc.frequency.value = accent ? 1760 : 1180;
    const level = (accent ? 0.09 : 0.055) * (countIn ? 1.2 : 1);
    env.gain.setValueAtTime(level, at);
    env.gain.exponentialRampToValueAtTime(0.0005, at + 0.035);
    osc.connect(env).connect(bus);
    osc.start(at);
    osc.stop(at + 0.05);
    osc.onended = () => env.disconnect();
  }
}
