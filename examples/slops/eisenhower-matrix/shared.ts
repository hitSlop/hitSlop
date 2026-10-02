import { type Zone } from "./schema";

export type QuadrantKey = Exclude<Zone, "inbox">;

export const QUADRANTS: { key: QuadrantKey; num: string; title: string; subtitle: string; tag: string; empty: string }[] = [
  { key: "q1", num: "I", title: "Do First", subtitle: "Urgent & Important", tag: "Crises & Deadlines", empty: "No fires on the blotter." },
  { key: "q2", num: "II", title: "Schedule", subtitle: "Not Urgent & Important", tag: "Focus & Leverage", empty: "Nothing to grow yet." },
  { key: "q3", num: "III", title: "Delegate", subtitle: "Urgent & Not Important", tag: "Interruptions", empty: "Nothing to hand off." },
  { key: "q4", num: "IV", title: "Don’t Do", subtitle: "Not Urgent & Not Important", tag: "Eliminate", empty: "Nothing to drop." },
];
