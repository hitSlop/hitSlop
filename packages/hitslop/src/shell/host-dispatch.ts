import type { HostRequest, HostCaptureResult } from "../wire/page";
import type { SlopPageHandle } from "./page-handle";

/** The only JavaScript entry point called by the native host. */
export function hostDispatcher(page: Pick<SlopPageHandle, "publish"> & Partial<SlopPageHandle>) {
  return async (request: HostRequest): Promise<true | HostCaptureResult> => {
    switch (request.method) {
      case "publish": page.publish(JSON.parse(request.payload)); break;
      case "capture.begin": {
        if (!page.capture) throw new Error("Document capture is not ready");
        return await page.capture.begin(request.token, request.mode);
      }
      case "capture.settle": {
        if (!page.capture) throw new Error("Document capture is not ready");
        return await page.capture.settle(request.token);
      }
      case "capture.restore": {
        if (!page.capture) throw new Error("Document capture is not ready");
        await page.capture.restore(request.token);
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
