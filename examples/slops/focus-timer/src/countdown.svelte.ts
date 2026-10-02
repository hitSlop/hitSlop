import { untrack } from "svelte";

export type TimerKind = "focus" | "rest";
export type Session = { startedAt: string; kind: TimerKind; seconds: number };
type Durations = { focusMinutes: number; restMinutes: number };

/** The live clock is transient; only finished sessions are saved. */
export function createCountdown(durations: () => Durations, onComplete: (session: Session) => void, now = Date.now) {
  const state = $state({
    kind: "focus" as TimerKind,
    remaining: 1500,
    duration: 1500,
    status: "ready" as "ready" | "running" | "paused" | "complete",
    completedKind: null as TimerKind | null,
  });
  let deadline = 0;
  let millisecondsLeft = 0;
  let startedAt: string | null = null;

  function prepare() {
    const settings = durations();
    state.duration = Math.max(1, state.kind === "focus" ? settings.focusMinutes : settings.restMinutes) * 60;
    state.remaining = state.duration;
    millisecondsLeft = state.duration * 1000;
    startedAt = null;
  }

  $effect(() => {
    const settings = durations();
    void settings.focusMinutes;
    void settings.restMinutes;
    untrack(() => {
      if (state.status === "ready" || state.status === "complete") prepare();
    });
  });

  function advance() {
    if (state.status !== "running") return;
    millisecondsLeft = Math.max(0, deadline - now());
    state.remaining = Math.ceil(millisecondsLeft / 1000);
    if (millisecondsLeft > 0) return;
    const completed = { startedAt: startedAt!, kind: state.kind, seconds: state.duration };
    state.completedKind = state.kind;
    state.kind = state.kind === "focus" ? "rest" : "focus";
    state.status = "complete";
    prepare();
    onComplete(completed);
  }

  function start() {
    if (state.status === "running") return;
    if (state.status !== "paused") prepare();
    startedAt ??= new Date(now()).toISOString();
    deadline = now() + millisecondsLeft;
    state.status = "running";
    state.completedKind = null;
  }

  function pause() {
    advance();
    if (state.status === "running") state.status = "paused";
  }

  function reset() {
    state.status = "ready";
    state.completedKind = null;
    prepare();
  }

  function select(kind: string) {
    if (state.status === "running" || (kind !== "focus" && kind !== "rest") || kind === state.kind) return;
    state.kind = kind;
    reset();
  }

  return { state, start, pause, reset, select, advance };
}
