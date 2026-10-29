import { SharingHost } from "../wire/constants.generated";
export const config = { host: SharingHost.origin, relay: SharingHost.origin };
export const frameOrigin = (copy: string) => `https://${copy}.${SharingHost.domain}`;
