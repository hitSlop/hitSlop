// Strudel ships plain JavaScript; this is the small surface the loop engine uses.
declare module "@strudel/core" {
  export type Pattern = {
    gain(value: number): Pattern;
    lpf(value: number): Pattern;
    room(value: number): Pattern;
    s(name: string): Pattern;
    queryArc(begin: number, end: number): unknown[];
  };
  export const s: (code: string) => Pattern;
  export const note: (code: string) => Pattern;
  export const stack: (...patterns: Pattern[]) => Pattern;
  export const silence: Pattern;
}
declare module "@strudel/mini" {
  /** Read every string passed to a pattern function as mini-notation. */
  export function miniAllStrings(): void;
}
declare module "@strudel/webaudio" {
  import type { Pattern } from "@strudel/core";
  export interface Repl {
    scheduler: { now(): number; setCps(cps: number): void };
    setPattern(pattern: Pattern, autostart?: boolean): Promise<unknown>;
    stop(): void;
  }
  export function webaudioRepl(options?: object): Repl;
  export function samples(source: string): Promise<void>;
  export function initAudio(options?: { disableWorklets?: boolean }): Promise<void>;
  export function registerSynthSounds(): void;
  export function getAudioContext(): AudioContext;
}
