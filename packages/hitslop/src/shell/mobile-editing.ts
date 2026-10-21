import type { Environment } from "../sdk/environment";

/** Native layout owns keyboard avoidance. This only reveals a newly obscured field;
 * it never resizes the page or interrupts caret/selection scrolling while typing. */
export function installMobileEditing(environment: Environment) {
  let pending = 0;
  const viewport = window.visualViewport;
  const reveal = () => {
    pending = 0;
    if (
      environment.current.platform !== "ios" ||
      document.documentElement.hasAttribute("data-slop-capture")
    )
      return;
    const field = document.activeElement;
    if (
      !(field instanceof HTMLElement) ||
      !field.matches(
        'textarea,input:not([type="checkbox"]):not([type="radio"]):not([type="file"]):not([type="button"]):not([type="submit"]),[contenteditable=""],[contenteditable="true"],[contenteditable="plaintext-only"]',
      )
    )
      return;
    const top = viewport?.offsetTop ?? 0;
    const bottom = top + (viewport?.height ?? window.innerHeight);
    const rect = field.getBoundingClientRect();
    // A large multiline editor needs native caret scrolling, not whole-field centering.
    if (rect.height > bottom - top) return;
    if (rect.top < top || rect.bottom > bottom)
      field.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "instant" });
  };
  const schedule = () => {
    if (!pending) pending = requestAnimationFrame(reveal);
  };
  document.addEventListener("focusin", schedule);
  window.addEventListener("resize", schedule);
  viewport?.addEventListener("resize", schedule);
  return () => {
    cancelAnimationFrame(pending);
    pending = 0;
    document.removeEventListener("focusin", schedule);
    window.removeEventListener("resize", schedule);
    viewport?.removeEventListener("resize", schedule);
  };
}
