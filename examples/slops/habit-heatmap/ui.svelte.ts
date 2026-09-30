import { dayKey } from "./calendar";
export const ui = $state({
  today: dayKey(new Date()),
  notice: "",
  highlightedDay: null as string | null,
});
