import { createEnvironment } from "../sdk/environment";
import type { HostEnvironment } from "../wire/page.generated";
import { installMobileEditing } from "./mobile-editing";

/** Install before app mount. Static development views have the same neutral attributes
 * as real captures, even when their viewport changes or a preset changes behind them. */
export function installEnvironment(identity: HostEnvironment) {
  const state = createEnvironment(identity);
  const root = document.documentElement;
  const attributes = () => {
    if (root.hasAttribute("data-pop-capture")) {
      delete root.dataset.popPlatform;
      delete root.dataset.popDevice;
      delete root.dataset.popLayout;
    } else {
      const current = state.environment.current;
      root.dataset.popPlatform = current.platform;
      root.dataset.popDevice = current.device;
      root.dataset.popLayout = current.layout;
    }
  };
  const unsubscribe = state.environment.subscribe(attributes);
  const observer = new MutationObserver(attributes);
  observer.observe(root, { attributes: true, attributeFilter: ["data-pop-capture"] });
  attributes();
  const editing = installMobileEditing(state.environment);
  return {
    ...state,
    /** The host's report of the height the keyboard covers (`__pop.keyboard`). */
    keyboard: editing.keyboard,
    /** The host's report of the safe areas the page runs under (`__pop.safeArea`). */
    safeArea: editing.safeArea,
    dispose() {
      editing.dispose();
      observer.disconnect();
      unsubscribe();
      state.dispose();
      delete root.dataset.popPlatform;
      delete root.dataset.popDevice;
      delete root.dataset.popLayout;
    },
  };
}
