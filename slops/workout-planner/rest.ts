import { remaining } from "./workout";

export function createRestClock(now: () => number = Date.now) {
  let deadline = 0;
  let total = 0;
  return {
    start(seconds: number) {
      total = Math.max(1, Math.round(seconds));
      deadline = now() + total * 1000;
    },
    extend(seconds: number) {
      if (deadline) {
        deadline += seconds * 1000;
        total += seconds;
      }
    },
    stop() {
      deadline = 0;
      total = 0;
    },
    read() {
      return { seconds: deadline ? remaining(deadline, now()) : 0, total };
    },
  };
}
