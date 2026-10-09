import { onDestroy, untrack } from "svelte";
import { cubicOut } from "svelte/easing";
import { Tween, prefersReducedMotion } from "svelte/motion";
import { current as context } from "./context";

/**
 * A number that eases toward `target()` whenever it changes: `const fill = motion(() =>
 * ratio)`, then render `fill.current`. It starts at the target, jumps under reduced
 * motion, and settles before a capture. Call it while a component
 * initializes.
 */
export function motion(target: () => number, options: { duration?: number; easing?: (t: number) => number } = {}) {
  const duration = options.duration ?? 280;
  const tween = new Tween(untrack(target), { duration, easing: options.easing ?? cubicOut });
  // Tween has no public cancel method. A zero-duration write of the current value
  // aborts its animation task without finishing an invisible animation on teardown.
  onDestroy(() => { void tween.set(tween.current, { duration: 0 }); });
  const settle = () => tween.set(tween.target, { duration: 0 });
  $effect(() => {
    const next = target();
    void tween.set(next, { duration: prefersReducedMotion.current ? 0 : duration });
  });
  $effect(() => context().capture.onPrepare(settle));
  return {
    get current() {
      return tween.current;
    },
  };
}
