// One notice at a time, shown by Root's outlet: refusals and the app's own messages.
let shown = $state<{ text: string; key: number }>();
let timer: ReturnType<typeof setTimeout> | undefined;
let count = 0;

/** Shows `text` in the window's notice region (`[data-slop-notice]`), replacing the
 * current one, for `duration` milliseconds: "3 tasks filed." Refusals arrive here too. */
export function notify(text: string, { duration = 4000 }: { duration?: number } = {}) {
  clearTimeout(timer);
  shown = { text, key: ++count };
  timer = setTimeout(() => (shown = undefined), duration);
}
export const notice = {
  get current() {
    return shown;
  },
  clear() {
    clearTimeout(timer);
    shown = undefined;
  },
};
