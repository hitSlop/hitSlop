import { call } from "./bridge";
import type { Haptics } from "../sdk/abi";
import type { HapticFeedback } from "../wire/page.generated";

/** Feedback never joins the document's edit/save barrier or reports a failed edit. */
export function ownerHaptics(): Haptics {
  const play = (feedback: HapticFeedback) => {
    void call({ method: "haptics.play", feedback }).catch(() => {});
  };
  return {
    selection: () => play({ kind: "selection" }),
    impact: (style = "light") => play({ kind: "impact", style }),
    notification: (notification) => play({ kind: "notification", notification }),
  };
}
