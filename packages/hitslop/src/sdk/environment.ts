import { CompactLayoutWidth } from "../schema/constants";
import type { HostEnvironment, HostLayout } from "../wire/page.generated";

export type EnvironmentSnapshot = Readonly<HostEnvironment & { layout: HostLayout }>;
export interface Environment {
  readonly current: EnvironmentSnapshot;
  /** Observes presentation changes; returns an idempotent disposer. */
  subscribe(listener: () => void): () => void;
}

/** Shared viewport behavior for the page shell and the SDK's older-host fallback.
 * Identity is supplied by the host, never inferred from a user agent or window size. */
export function createEnvironment(
  identity: HostEnvironment = { platform: "unknown", device: "unknown" },
) {
  const regular = matchMedia(`(min-width: ${CompactLayoutWidth}px)`);
  let host = identity;
  const snapshot = (): EnvironmentSnapshot =>
    Object.freeze({
      platform: host.platform,
      device: host.device,
      layout: regular.matches ? "regular" : "compact",
    });
  let value = snapshot();
  const listeners = new Set<() => void>();
  const publish = () => {
    const next = snapshot();
    if (
      next.platform === value.platform &&
      next.device === value.device &&
      next.layout === value.layout
    )
      return;
    value = next;
    for (const listener of listeners) listener();
  };
  regular.addEventListener("change", publish);
  const environment: Environment = Object.freeze({
    get current() {
      return value;
    },
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  });
  return {
    environment,
    setHost(identity: HostEnvironment) {
      host = identity;
      publish();
    },
    dispose() {
      regular.removeEventListener("change", publish);
      listeners.clear();
    },
  };
}
