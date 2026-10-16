import type { HostRequest } from "../wire/page";
import type { SlopPageHandle } from "./page-handle";
import type { ClosePreparation } from "../wire/page.generated";

/** The only JavaScript entry point called by the native host. */
export function hostDispatcher(page: Pick<SlopPageHandle, "publish"> & Partial<SlopPageHandle>) {
  return async (request: HostRequest): Promise<true | ClosePreparation> => {
    switch (request.method) {
      case "prepareClose": {
        if (!page.prepareClose) throw new Error("Document prepareClose is not ready");
        return page.prepareClose();
      }
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
