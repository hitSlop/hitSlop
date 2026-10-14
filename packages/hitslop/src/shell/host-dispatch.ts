import type { HostRequest } from "../wire/page";
import type { SlopPageHandle } from "./page-handle";

/** The only JavaScript entry point called by the native host. */
export function hostDispatcher(page: Pick<SlopPageHandle, "publish"> & Partial<SlopPageHandle>) {
  return async (request: HostRequest): Promise<true> => {
    switch (request.method) {
      case "publish": {
        const pushes: unknown = JSON.parse(request.payload);
        page.publish(pushes);
        break;
      }
      default: {
        const action = page[request.method];
        if (!action) throw new Error(`Document ${request.method} is not ready`);
        await action();
      }
    }
    return true;
  };
}
