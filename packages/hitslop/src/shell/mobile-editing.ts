import type { Environment } from "../sdk/environment";

const INSET = "--slop-keyboard-inset";
const ROOM = "data-slop-keyboard-room";
const MARGIN = 12;
const TEXT_FIELD =
  'textarea,input:not([type="checkbox"]):not([type="radio"]):not([type="file"]):not([type="button"]):not([type="submit"]),[contenteditable=""],[contenteditable="true"],[contenteditable="plaintext-only"]';

/** Something that scrolls the field, and the element that grows when it needs more room:
 * the scroller itself, or for the page, the app element that holds the field. */
type Scroller = { element: Element; room: HTMLElement | undefined };

/** On the iPhone the keyboard covers the page; the page never resizes. The host reports the
 * covered height through the returned `keyboard(inset)`, which publishes it as
 * `--slop-keyboard-inset`. The shell brings a newly focused field above the keyboard with no
 * help from the app: it scrolls the field's containers, gives the nearest one temporary room
 * at its end when it cannot scroll far enough, and as a last resort shifts the page up, as
 * Safari does. Everything it changed is undone when the keyboard hides or focus moves. It
 * never interrupts caret/selection scrolling while typing. It also gives `:active` styles a
 * touch listener to run on and number fields a numeric keypad. */
export function installMobileEditing(environment: Environment) {
  const ios = () => environment.current.platform === "ios";
  let pending = 0;
  let inset = 0;
  const viewport = window.visualViewport;
  /** Inline styles this module set, with the values to put back. */
  const changed: {
    element: HTMLElement;
    property: "padding-bottom" | "translate";
    value: string;
  }[] = [];
  const set = (element: HTMLElement, property: "padding-bottom" | "translate", value: string) => {
    changed.push({ element, property, value: element.style.getPropertyValue(property) });
    element.style.setProperty(property, value);
    if (property === "padding-bottom") element.setAttribute(ROOM, "");
  };
  const restore = () => {
    for (const { element, property, value } of changed.splice(0).reverse()) {
      if (value) element.style.setProperty(property, value);
      else element.style.removeProperty(property);
      element.removeAttribute(ROOM);
    }
  };
  const reveal = () => {
    pending = 0;
    restore();
    if (!ios() || document.documentElement.hasAttribute("data-slop-capture")) return;
    const field = document.activeElement;
    if (!(field instanceof HTMLElement) || !field.matches(TEXT_FIELD)) return;
    const viewTop = viewport?.offsetTop ?? 0;
    // The layout viewport stays full height under the keyboard, so the visible band ends
    // where the keyboard and its toolbar begin.
    const keyboardTop = window.innerHeight - inset;
    let rect = field.getBoundingClientRect();
    // A large multiline editor needs native caret scrolling, not whole-field centering.
    if (rect.height > keyboardTop - viewTop) return;
    const x = rect.left + Math.min(rect.width / 2, 24);
    scrollers(field).forEach(({ element, room }, index) => {
      const box =
        element === document.scrollingElement ? undefined : element.getBoundingClientRect();
      const top = Math.max(viewTop, box?.top ?? viewTop);
      const bottom = uncovered(
        field,
        element,
        x,
        Math.min(keyboardTop, box?.bottom ?? keyboardTop),
      );
      if (rect.height > bottom - top) return;
      const delta =
        rect.bottom > bottom
          ? rect.bottom - bottom + MARGIN
          : rect.top < top
            ? rect.top - top - MARGIN
            : 0;
      if (!delta) return;
      const available =
        delta > 0
          ? element.scrollHeight - element.clientHeight - element.scrollTop
          : -element.scrollTop;
      // Only the field's nearest container grows, and only by what it lacks.
      if (inset > 0 && delta > available && index === 0 && room) {
        const padding = parseFloat(getComputedStyle(room).paddingBottom) || 0;
        set(room, "padding-bottom", `${padding + delta - available}px`);
      }
      element.scrollBy({ top: delta, behavior: "instant" });
      rect = field.getBoundingClientRect();
    });
    // Nothing could scroll it far enough: shift the page up just enough, keeping the field's
    // top in view.
    if (inset > 0 && rect.bottom > keyboardTop) {
      const shift = Math.min(
        rect.bottom - keyboardTop + MARGIN,
        Math.max(0, rect.top - viewTop - MARGIN),
      );
      if (shift >= 1) set(document.body, "translate", `0 ${-shift}px`);
    }
  };
  const schedule = () => {
    if (!pending) pending = requestAnimationFrame(reveal);
  };
  const focusin = (event: FocusEvent) => {
    if (ios()) numericKeypad(event.target);
    schedule();
  };
  const touch = () => {};
  document.addEventListener("focusin", focusin);
  window.addEventListener("resize", schedule);
  viewport?.addEventListener("resize", schedule);
  // iOS WebKit applies `:active` only to pages with a touch listener.
  if (ios()) document.addEventListener("touchstart", touch, { passive: true });
  return {
    /** The status bar and home indicator heights the edge-to-edge page extends under. */
    safeArea(top: number, bottom: number) {
      const px = (value: number) => `${Number.isFinite(value) ? Math.max(0, value) : 0}px`;
      document.documentElement.style.setProperty("--slop-safe-top", px(top));
      document.documentElement.style.setProperty("--slop-safe-bottom", px(bottom));
      if (inset) schedule();
    },
    keyboard(next: number) {
      const value = Number.isFinite(next) ? Math.max(0, next) : 0;
      document.documentElement.style.setProperty(INSET, `${value}px`);
      if (value === inset) return;
      inset = value;
      if (!inset) {
        cancelAnimationFrame(pending);
        pending = 0;
        restore();
      } else schedule();
    },
    dispose() {
      cancelAnimationFrame(pending);
      pending = 0;
      restore();
      document.removeEventListener("focusin", focusin);
      document.removeEventListener("touchstart", touch);
      window.removeEventListener("resize", schedule);
      viewport?.removeEventListener("resize", schedule);
    },
  };
}

/** The containers that can scroll `field`, nearest first. A fixed ancestor (a dialog) ends
 * the walk, since scrolling anything outside it cannot move the field. The page comes last
 * when it scrolls, growing through the app element that holds the field. */
function scrollers(field: HTMLElement): Scroller[] {
  const found: Scroller[] = [];
  let node = field.parentElement;
  for (
    ;
    node && node !== document.body && node !== document.documentElement;
    node = node.parentElement
  ) {
    const style = getComputedStyle(node);
    if (style.overflowY === "auto" || style.overflowY === "scroll")
      found.push({ element: node, room: node });
    if (style.position === "fixed") return found;
  }
  const page = document.scrollingElement;
  const locked = [document.documentElement, document.body].some((element) =>
    ["hidden", "clip"].includes(getComputedStyle(element).overflowY),
  );
  if (page && !locked) {
    const app = document.querySelector("[data-slop-root]");
    const room = app && [...app.children].find((child) => child.contains(field));
    found.push({ element: page, room: room instanceof HTMLElement ? room : undefined });
  }
  return found;
}

/** `bottom`, raised above anything pinned inside `scroller` that covers the field's column,
 * such as a dialog's sticky Save bar. */
function uncovered(field: HTMLElement, scroller: Element, x: number, bottom: number) {
  for (let tries = 0; tries < 3; tries++) {
    const hit = document.elementFromPoint(x, bottom - 1);
    if (!hit || field.contains(hit) || hit.contains(field) || !scroller.contains(hit))
      return bottom;
    let pinned: Element | null = hit;
    while (
      pinned &&
      pinned !== scroller &&
      !["sticky", "fixed"].includes(getComputedStyle(pinned).position)
    )
      pinned = pinned.parentElement;
    if (!pinned || pinned === scroller) return bottom;
    const top = pinned.getBoundingClientRect().top;
    if (top >= bottom - 1) return bottom;
    bottom = top;
  }
  return bottom;
}

/** A number field with a non-negative minimum gets the numeric keypad (the decimal one for
 * fractional steps) unless the author chose an `inputmode`; the pad has no minus key. */
function numericKeypad(target: EventTarget | null) {
  if (!(target instanceof HTMLInputElement) || target.type !== "number") return;
  if (target.hasAttribute("inputmode")) return;
  const min = target.getAttribute("min");
  if (min === null || !(Number(min) >= 0)) return;
  const step = target.getAttribute("step");
  const whole = step === null || (step !== "any" && Number.isInteger(Number(step)));
  target.inputMode = whole ? "numeric" : "decimal";
}
