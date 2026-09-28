import { CHANNEL_IDS, type ChannelID, type ChannelLevels } from "./schema";

function clamp01(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.min(1, Math.max(0, value));
}

function audioContextClass(): typeof AudioContext | undefined {
  return window.AudioContext || (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
}

function noiseBuffer(ctx: AudioContext): AudioBuffer {
  const length = Math.floor(ctx.sampleRate * 2);
  const buffer = ctx.createBuffer(1, length, ctx.sampleRate);
  const data = buffer.getChannelData(0);
  for (let i = 0; i < length; i += 1) data[i] = Math.random() * 2 - 1;
  return buffer;
}

export class AmbientEngine {
  private ctx: AudioContext | null = null;
  private master: GainNode | null = null;
  private channels: Partial<Record<ChannelID, GainNode>> = {};
  private thunderFilter: BiquadFilterNode | null = null;
  private thunderBurst: GainNode | null = null;
  private timers: ReturnType<typeof setTimeout>[] = [];
  private playing = false;
  private closed = false;

  get isOpen(): boolean {
    return this.ctx !== null && this.ctx.state !== "closed";
  }

  async ensure(): Promise<boolean> {
    if (this.closed) return false;
    try {
      if (!this.ctx) this.build();
      if (this.ctx?.state === "suspended") await this.ctx.resume();
      return this.isOpen;
    } catch {
      return false;
    }
  }

  setMix(playing: boolean, master: number, gains: ChannelLevels): void {
    this.playing = playing;
    if (!this.ctx || !this.master) return;
    const now = this.ctx.currentTime;
    this.master.gain.setTargetAtTime(playing ? clamp01(master / 100) : 0, now, 0.05);
    for (const id of CHANNEL_IDS) {
      const node = this.channels[id];
      if (!node) continue;
      node.gain.setTargetAtTime(playing ? clamp01(gains[id] / 100) * 0.2 : 0, now, 0.04);
    }
  }

  close(): void {
    this.closed = true;
    this.playing = false;
    this.clearTimers();
    const ctx = this.ctx;
    this.ctx = null;
    this.master = null;
    this.channels = {};
    this.thunderFilter = null;
    this.thunderBurst = null;
    if (ctx && ctx.state !== "closed") void ctx.close().catch(() => undefined);
  }

  private clearTimers(): void {
    for (const timer of this.timers) clearTimeout(timer);
    this.timers = [];
  }

  private later(delay: number, work: () => void): void {
    const timer = setTimeout(() => {
      this.timers = this.timers.filter((item) => item !== timer);
      if (this.closed || !this.ctx) return;
      work();
    }, delay);
    this.timers.push(timer);
  }

  private build(): void {
    const Ctx = audioContextClass();
    if (!Ctx) return;
    const ctx = new Ctx();
    this.ctx = ctx;
    const master = ctx.createGain();
    master.gain.value = 0;
    master.connect(ctx.destination);
    this.master = master;

    const buffer = noiseBuffer(ctx);

    const rain = ctx.createBufferSource();
    rain.buffer = buffer;
    rain.loop = true;
    const rainHigh = ctx.createBiquadFilter();
    rainHigh.type = "highpass";
    rainHigh.frequency.value = 380;
    const rainLow = ctx.createBiquadFilter();
    rainLow.type = "lowpass";
    rainLow.frequency.value = 1400;
    const rainGain = ctx.createGain();
    rainGain.gain.value = 0;
    rain.connect(rainHigh);
    rainHigh.connect(rainLow);
    rainLow.connect(rainGain);
    rainGain.connect(master);
    rain.start(0);
    this.channels.rain = rainGain;

    const wind = ctx.createBufferSource();
    wind.buffer = buffer;
    wind.loop = true;
    const windFilter = ctx.createBiquadFilter();
    windFilter.type = "bandpass";
    windFilter.frequency.value = 380;
    windFilter.Q.value = 1.4;
    const windGain = ctx.createGain();
    windGain.gain.value = 0;
    const lfo = ctx.createOscillator();
    lfo.type = "sine";
    lfo.frequency.value = 0.09;
    const lfoDepth = ctx.createGain();
    lfoDepth.gain.value = 220;
    lfo.connect(lfoDepth);
    lfoDepth.connect(windFilter.frequency);
    wind.connect(windFilter);
    windFilter.connect(windGain);
    windGain.connect(master);
    wind.start(0);
    lfo.start(0);
    this.channels.wind = windGain;

    const thunder = ctx.createBufferSource();
    thunder.buffer = buffer;
    thunder.loop = true;
    const thunderFilter = ctx.createBiquadFilter();
    thunderFilter.type = "lowpass";
    thunderFilter.frequency.value = 110;
    thunderFilter.Q.value = 0.7;
    const thunderBurst = ctx.createGain();
    thunderBurst.gain.value = 1;
    const thunderGain = ctx.createGain();
    thunderGain.gain.value = 0;
    thunder.connect(thunderFilter);
    thunderFilter.connect(thunderBurst);
    thunderBurst.connect(thunderGain);
    thunderGain.connect(master);
    thunder.start(0);
    this.channels.thunder = thunderGain;
    this.thunderFilter = thunderFilter;
    this.thunderBurst = thunderBurst;

    const birdsGain = ctx.createGain();
    birdsGain.gain.value = 0;
    birdsGain.connect(master);
    this.channels.birds = birdsGain;

    const nightGain = ctx.createGain();
    nightGain.gain.value = 0;
    nightGain.connect(master);
    this.channels.night = nightGain;

    this.armThunder();
    this.armBirds();
    this.armNight();
  }

  private armThunder(): void {
    this.later(4000 + Math.random() * 8000, () => {
      if (this.playing) this.crack();
      this.armThunder();
    });
  }

  private armBirds(): void {
    this.later(900 + Math.random() * 2800, () => {
      if (this.playing) this.chirp();
      this.armBirds();
    });
  }

  private armNight(): void {
    this.later(600 + Math.random() * 900, () => {
      if (this.playing) this.cricket();
      this.armNight();
    });
  }

  private crack(): void {
    const ctx = this.ctx;
    const filter = this.thunderFilter;
    const burst = this.thunderBurst;
    if (!ctx || !filter || !burst) return;
    const now = ctx.currentTime;
    filter.frequency.cancelScheduledValues(now);
    filter.frequency.setValueAtTime(110, now);
    filter.frequency.exponentialRampToValueAtTime(420, now + 0.08);
    filter.frequency.exponentialRampToValueAtTime(90, now + 1.8);
    burst.gain.cancelScheduledValues(now);
    burst.gain.setValueAtTime(1, now);
    burst.gain.linearRampToValueAtTime(3.1, now + 0.1);
    burst.gain.exponentialRampToValueAtTime(1, now + 1.6);
  }

  private chirp(): void {
    const ctx = this.ctx;
    const bus = this.channels.birds;
    if (!ctx || !bus) return;
    const osc = ctx.createOscillator();
    const env = ctx.createGain();
    osc.type = "sine";
    const now = ctx.currentTime;
    const start = 2200 + Math.random() * 1400;
    osc.frequency.setValueAtTime(start, now);
    osc.frequency.exponentialRampToValueAtTime(start * 1.32, now + 0.05);
    osc.frequency.exponentialRampToValueAtTime(start * 0.88, now + 0.14);
    env.gain.setValueAtTime(0.0001, now);
    env.gain.exponentialRampToValueAtTime(0.22, now + 0.012);
    env.gain.exponentialRampToValueAtTime(0.0001, now + 0.16);
    osc.connect(env);
    env.connect(bus);
    osc.start(now);
    osc.stop(now + 0.18);
  }

  private cricket(): void {
    const ctx = this.ctx;
    const bus = this.channels.night;
    if (!ctx || !bus) return;
    const now = ctx.currentTime;
    const pulses = 3 + Math.floor(Math.random() * 3);
    for (let i = 0; i < pulses; i += 1) {
      const osc = ctx.createOscillator();
      const env = ctx.createGain();
      osc.type = "sine";
      osc.frequency.value = 4100 + Math.random() * 500;
      const t = now + i * 0.038;
      env.gain.setValueAtTime(0.0001, t);
      env.gain.exponentialRampToValueAtTime(0.16, t + 0.006);
      env.gain.exponentialRampToValueAtTime(0.0001, t + 0.024);
      osc.connect(env);
      env.connect(bus);
      osc.start(t);
      osc.stop(t + 0.03);
    }
  }
}
