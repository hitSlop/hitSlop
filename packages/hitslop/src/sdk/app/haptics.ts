import { current } from "./context";
import type { Haptics } from "../abi";
export type { HapticImpactStyle, HapticNotification } from "../abi";

/** Native tactile feedback for deliberate editor actions. Silent on older or unsupported
 * hosts. Play success after an accepted write; never from a document observer or render. */
export const haptics: Haptics = {
  selection() {
    current().haptics?.selection();
  },
  impact(style = "light") {
    current().haptics?.impact(style);
  },
  notification(notification) {
    current().haptics?.notification(notification);
  },
};
