import { note, s, silence, stack, type Pattern } from "@strudel/core";
import { miniAllStrings } from "@strudel/mini";
import { getAudioContext, initAudio, registerSynthSounds, samples, webaudioRepl, type Repl } from "@strudel/webaudio";
import type { Track } from "./schema";

export type Sounds = "loading" | "ready" | "offline";

/** Turn the saved tracks into one Strudel pattern. A track whose notation can't be read is skipped and reported by id. */
export function build(tracks: readonly Track[]): { pattern: Pattern; errors: Record<string, string> } {
  const errors: Record<string, string> = {};
  const layers: Pattern[] = [];
  for (const track of tracks) {
    if (track.muted) continue;
    try {
      const base = track.kind === "drums" ? s(track.code) : note(track.code).s(track.voice);
      const pattern = base.gain(track.gain).lpf(track.filter).room(track.room);
      pattern.queryArc(0, 1); // parse now so a typo shows up on its track, not as silence
      layers.push(pattern);
    } catch {
      // Strudel's parser message is long and technical; say what to look for.
      errors[track.$id] = "check for an unclosed bracket or a stray symbol.";
    }
  }
  return { pattern: layers.length ? stack(...layers) : silence, errors };
}

/** Strudel's scheduler and audio engine. Loaded on the first Play so nothing makes sound or touches the network before then. */
export class LoopEngine {
  private readonly repl: Repl;
  sounds: Sounds = "loading";

  static async create(onSounds: (sounds: Sounds) => void): Promise<LoopEngine> {
    // Worklet effects load from data: URLs, which the slop CSP blocks; the oscillators, samples, filter and reverb do not need them.
    await initAudio({ disableWorklets: true });
    registerSynthSounds();
    miniAllStrings();
    const engine = new LoopEngine();
    // The default drum samples come from the Strudel community's public repository; synths work without them.
    // Wait a few seconds so the first bar isn't silent, but never block Play on a slow network.
    const loaded = samples("github:tidalcycles/dirt-samples").then(
      () => { engine.sounds = "ready"; },
      () => { engine.sounds = "offline"; },
    );
    await Promise.race([loaded, new Promise((resolve) => setTimeout(resolve, 6000))]);
    onSounds(engine.sounds);
    void loaded.then(() => onSounds(engine.sounds));
    return engine;
  }

  private constructor() {
    this.repl = webaudioRepl();
  }

  async play(tracks: readonly Track[], bpm: number): Promise<Record<string, string>> {
    this.repl.scheduler.setCps(bpm / 60 / 4);
    const { pattern, errors } = build(tracks);
    await this.repl.setPattern(pattern, true);
    return errors;
  }

  stop(): void {
    this.repl.stop();
  }

  /** Where we are in the current cycle, 0 to 1. */
  position(): number {
    const now = this.repl.scheduler.now();
    return now - Math.floor(now);
  }

  dispose(): void {
    this.repl.stop();
    void getAudioContext().suspend();
  }
}
