import type * as ToneNamespace from "tone";
import { STEPS, bassNotes, type Kit, type Pattern } from "./pattern";

type Tone = typeof ToneNamespace;
let replacedDefault = false;

interface Sound {
  wave: "sine" | "triangle" | "square";
  kickPitch: number;
  kickDecay: number;
  snareDecay: number;
  snareCutoff: number;
  hatDecay: number;
  hatCutoff: number;
  bassCutoff: number;
  warmth: number;
}
const SOUNDS: Record<Kit, Sound> = {
  "808": { wave: "sine", kickPitch: 0.05, kickDecay: 0.55, snareDecay: 0.2, snareCutoff: 1800, hatDecay: 0.05, hatCutoff: 7000, bassCutoff: 700, warmth: 14000 },
  lofi: { wave: "triangle", kickPitch: 0.04, kickDecay: 0.35, snareDecay: 0.16, snareCutoff: 2600, hatDecay: 0.04, hatCutoff: 4500, bassCutoff: 500, warmth: 3200 },
  chip: { wave: "square", kickPitch: 0.015, kickDecay: 0.18, snareDecay: 0.1, snareCutoff: 5000, hatDecay: 0.025, hatCutoff: 9000, bassCutoff: 1400, warmth: 14000 },
};

/** The sequencer behind the pad. Tone is loaded on the first Play, so no audio exists until someone asks for it. */
export class BeatEngine {
  private readonly transport: ReturnType<Tone["getTransport"]>;
  private readonly nodes: ToneNamespace.ToneAudioNode[] = [];
  private voices!: { kick(t: number): void; snare(t: number): void; hat(t: number): void; bass(note: string, t: number): void };
  private pattern: Pattern;
  private counter = 0;
  private loop: number;

  static async create(pattern: Pattern, kit: Kit, onStep: (step: number | null) => void): Promise<BeatEngine> {
    const Tone = await import("tone");
    // The default clock runs in a blob Worker, which the slop CSP blocks; a timer-driven clock works everywhere.
    const context = new Tone.Context({ clockSource: "timeout" });
    // Replacing the default context (created behind the scenes with a Worker clock) closes it the first time.
    Tone.setContext(context, !replacedDefault);
    replacedDefault = true;
    await Tone.start();
    return new BeatEngine(Tone, context, pattern, kit, onStep);
  }

  private constructor(
    private readonly Tone: Tone,
    private readonly context: ToneNamespace.Context,
    pattern: Pattern,
    kit: Kit,
    private readonly onStep: (step: number | null) => void,
  ) {
    this.pattern = pattern;
    this.transport = Tone.getTransport();
    this.transport.swingSubdivision = "16n";
    this.build(kit);
    this.loop = this.transport.scheduleRepeat((time) => {
      const step = this.counter++ % STEPS;
      const p = this.pattern;
      if (p.kick[step] === "x") this.voices.kick(time);
      if (p.snare[step] === "x") this.voices.snare(time);
      if (p.hat[step] === "x") this.voices.hat(time);
      const note = Number(p.bass[step]);
      if (note >= 1 && note <= bassNotes.length) this.voices.bass(bassNotes[note - 1]!, time);
      Tone.getDraw().schedule(() => this.onStep(step), time);
    }, "16n");
  }

  setPattern(pattern: Pattern): void { this.pattern = pattern; }

  setTempo(bpm: number, swing: number): void {
    this.transport.bpm.value = bpm;
    this.transport.swing = swing / 100;
  }

  setKit(kit: Kit): void {
    this.disposeVoices();
    this.build(kit);
  }

  start(): void {
    this.counter = 0;
    this.transport.start("+0.05");
  }

  stop(): void {
    this.transport.stop();
    this.counter = 0;
    this.onStep(null);
  }

  dispose(): void {
    this.transport.stop();
    this.transport.clear(this.loop);
    this.disposeVoices();
    void this.context.dispose();
  }

  private disposeVoices(): void {
    for (const node of this.nodes.splice(0)) node.dispose();
  }

  private build(kit: Kit): void {
    const { Tone } = this;
    const sound = SOUNDS[kit];
    const own = <T extends ToneNamespace.ToneAudioNode>(node: T): T => (this.nodes.push(node), node);

    const limiter = own(new Tone.Limiter(-2)).toDestination();
    const warmth = own(new Tone.Filter(sound.warmth, "lowpass")).connect(limiter);
    const kick = own(new Tone.MembraneSynth({
      pitchDecay: sound.kickPitch,
      octaves: 6,
      oscillator: { type: sound.wave },
      envelope: { attack: 0.001, decay: sound.kickDecay, sustain: 0, release: 0.1 },
    })).connect(warmth);
    kick.volume.value = -4;
    const snareFilter = own(new Tone.Filter(sound.snareCutoff, kit === "lofi" ? "lowpass" : "bandpass")).connect(warmth);
    const snare = own(new Tone.NoiseSynth({ noise: { type: "white" }, envelope: { attack: 0.001, decay: sound.snareDecay, sustain: 0 } })).connect(snareFilter);
    snare.volume.value = -9;
    const hatFilter = own(new Tone.Filter(sound.hatCutoff, "highpass")).connect(warmth);
    const hat = own(new Tone.NoiseSynth({ noise: { type: "white" }, envelope: { attack: 0.001, decay: sound.hatDecay, sustain: 0 } })).connect(hatFilter);
    hat.volume.value = -20;
    const bass = own(new Tone.MonoSynth({
      oscillator: { type: sound.wave },
      envelope: { attack: 0.005, decay: 0.25, sustain: 0.35, release: 0.12 },
      filter: { Q: 2, type: "lowpass", rolloff: -12 },
      filterEnvelope: { attack: 0.005, decay: 0.2, sustain: 0.3, baseFrequency: sound.bassCutoff * 0.3, octaves: 2 },
    })).connect(warmth);
    bass.volume.value = -8;

    this.voices = {
      kick: (time) => kick.triggerAttackRelease("C1", "8n", time),
      snare: (time) => snare.triggerAttackRelease("16n", time),
      hat: (time) => hat.triggerAttackRelease("32n", time),
      bass: (note, time) => bass.triggerAttackRelease(note, "16n", time),
    };
  }
}
