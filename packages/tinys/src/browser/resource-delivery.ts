import { BrowserHost } from "../schema/constants";
import { config, frameOrigin } from "../sharing/config";

/** Local authored frames fetch through the owner’s Service Worker channel. Hosted
 * frames use immutable network resources authorized by their HttpOnly cookie. */
const network = location.protocol === "https:";
export const resourceDelivery = {
  kind: network ? "network" : "serviceWorker",
  host: network ? config.host : `http://127.0.0.1:${BrowserHost.port}`,
  frameOrigin: (copy: string) =>
    network ? frameOrigin(copy) : `http://${copy}.localhost:${BrowserHost.port}`,
} as const;
