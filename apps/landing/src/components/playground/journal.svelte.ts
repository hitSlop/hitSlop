// One shared Little Days journal for the hero: the Mac window and the iPhone both read and write it,
// so a tap on either one shows up on the other, the way two copies of a live pop stay in step.
export type Kind = "feed" | "diaper" | "sleep" | "note";
export type Person = "Dad" | "Mia" | "Grandma";
export type Entry = { id: number; kind: Kind; time: string; text: string; who: Person };

export const kinds: Record<Kind, { label: string; icon: string; tone: string; text: string }> = {
  feed: { label: "Feed", icon: "🍼", tone: "#ffe1cf", text: "Bottle · 120 mL" },
  diaper: { label: "Diaper", icon: "💧", tone: "#d9ebff", text: "Wet, fresh and happy" },
  sleep: { label: "Sleep", icon: "🌙", tone: "#e6dcff", text: "Down for a nap" },
  note: { label: "Memory", icon: "⭐", tone: "#fff0b3", text: "Grabbed her toes today!" },
};

export const faces: Record<Person, string> = {
  Dad: "/assets/brand/face-dad.webp",
  Mia: "/assets/brand/face-mia.webp",
  Grandma: "/assets/brand/face-grandma.webp",
};

export const journal = $state<{ entries: Entry[]; toast: { id: number; who: Person; label: string } | null }>({
  entries: [
    { id: 4, kind: "feed", time: "8:30 AM", text: "Bottle · 120 mL", who: "Dad" },
    { id: 3, kind: "diaper", time: "7:55 AM", text: "Wet, fresh and happy", who: "Mia" },
    { id: 2, kind: "sleep", time: "5:10 AM", text: "Slept 3h 20m", who: "Mia" },
    { id: 1, kind: "note", time: "Yesterday", text: "First belly laugh 💛", who: "Grandma" },
  ],
  toast: null,
});

let next = 10;
let toastTimer: ReturnType<typeof setTimeout> | undefined;
const clock = () => new Date().toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });

export function log(kind: Kind, who: Person): void {
  const id = next++;
  journal.entries = [{ id, kind, time: clock(), text: kinds[kind].text, who }, ...journal.entries].slice(0, 6);
  if (who !== "Dad") {
    journal.toast = { id, who, label: kinds[kind].label.toLowerCase() };
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => { journal.toast = null; }, 3200);
  }
}

// A couple of friends drop in an entry now and then, so the shared space feels alive. Starts once.
let started = false;
export function startFriends(): () => void {
  if (started || matchMedia("(prefers-reduced-motion: reduce)").matches) return () => {};
  started = true;
  const plan: Array<[number, Kind, Person]> = [[7000, "diaper", "Mia"], [19000, "feed", "Grandma"], [34000, "note", "Mia"]];
  const timers = plan.map(([delay, kind, who]) => setTimeout(() => log(kind, who), delay));
  return () => { timers.forEach(clearTimeout); started = false; };
}
