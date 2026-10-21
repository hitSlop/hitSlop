import { cubicOut } from "svelte/easing";
import { prefersReducedMotion } from "svelte/motion";
import { fade, fly, scale, type TransitionConfig } from "svelte/transition";

/** Surface motion preserves authored positioning transforms. Use on the actual DOM
 * element in a Bits UI forceMount/child snippet, with transition: for reversal. */
export function surfaceTransition(
  node: HTMLElement,
  { kind = "dialog" }: { kind?: "dialog" | "sheet" } = {},
): TransitionConfig {
  return fly(node, {
    y: kind === "sheet" ? node.getBoundingClientRect().height : 8,
    opacity: kind === "sheet" ? 1 : 0,
    duration: prefersReducedMotion.current ? 0 : kind === "sheet" ? 260 : 200,
    easing: cubicOut,
  });
}

export function overlayTransition(node: HTMLElement): TransitionConfig {
  return fade(node, { duration: prefersReducedMotion.current ? 0 : 160 });
}

/** Animate the floating content, leaving Bits UI's wrapperProps on its outer wrapper. */
export function popoverTransition(node: HTMLElement): TransitionConfig {
  return scale(node, {
    start: 0.98,
    duration: prefersReducedMotion.current ? 0 : 120,
    easing: cubicOut,
  });
}
