import type { SharingEvent } from "../wire/sharing.generated";

/**
 * What a hosted tab waits on before it mounts the page: the app's resources, and for a new
 * join the first catch-up. Each is settled by the worker's `sharing` events, by phase and
 * never by the words shown, and a stop settles both: a pause the person must act on, or the
 * room ending. A join that has stopped never mounts, so the host reports `stopped` instead of
 * waiting for a catch-up that cannot come.
 */
export class JoinGate {
  /** Why sharing stopped before the page mounted, in the words shown to a person. */
  stopped: string | undefined;
  /** The room no longer exists: the page opens on the local copy. */
  ended = false;
  readonly resources: Promise<void>;
  readonly catchup: Promise<void>;
  private resourcesReady = () => {};
  private caughtUp = () => {};

  constructor() {
    this.resources = new Promise((resolve) => (this.resourcesReady = resolve));
    this.catchup = new Promise((resolve) => (this.caughtUp = resolve));
  }

  handle(event: SharingEvent) {
    if (event.type === "resourcesReady") this.resourcesReady();
    if (event.type !== "sync") return;
    if (event.phase === "synced") this.caughtUp();
    else if (event.phase === "ended") {
      this.ended = true;
      this.resourcesReady();
      this.caughtUp();
    } else if (event.phase === "paused") {
      this.stopped ??= event.status;
      this.resourcesReady();
      this.caughtUp();
    }
  }
}
