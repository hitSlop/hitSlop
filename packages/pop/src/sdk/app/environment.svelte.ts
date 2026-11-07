import { getContext } from "svelte";
import type { Environment, EnvironmentSnapshot } from "../environment";

const key = Symbol("pop.environment");
export type { EnvironmentSnapshot } from "../environment";

/** One reactive view of the host's environment per mount. */
export function environmentContext(environment: Environment) {
  let snapshot = $state.raw(environment.current);
  const unsubscribe = environment.subscribe(() => {
    snapshot = environment.current;
  });
  const value = Object.freeze({
    get current(): EnvironmentSnapshot {
      return snapshot;
    },
  });
  return {
    context: new Map([[key, value]]),
    dispose: unsubscribe,
  };
}

/** Call during editor component initialization. Read `.current` reactively; platform
 * and device describe the host, while layout follows the actual document viewport. */
export function getEnvironment(): { readonly current: EnvironmentSnapshot } {
  let environment: ReturnType<typeof getEnvironment> | undefined;
  try {
    environment = getContext(key);
  } catch {
    throw new Error(
      "getEnvironment() must run while a component initializes, at the top of its <script>",
    );
  }
  if (!environment)
    throw new Error(
      "getEnvironment() needs a mounted pop editor; static views use their capture props",
    );
  return environment;
}
