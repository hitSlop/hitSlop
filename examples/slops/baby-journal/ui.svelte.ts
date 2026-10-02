import { dayKey } from "./journal";

// The day on screen and the child's photo the editor loaded; the export shows both.
export const ui = $state({
  selectedDay: dayKey(Date.now()),
  photoUrl: "",
});
