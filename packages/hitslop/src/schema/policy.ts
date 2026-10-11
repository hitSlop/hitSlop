import { NativeResourcePolicy, PreviewResourcePolicy, PreviewHostPolicy } from "../wire/constants.generated";
import type { Capability } from "../wire/app.generated";
/** Vite serves every source module under /__app__; attachment URLs never match it. */
export function appContentSecurityPolicy(environment: "native" | "browser", origin = "http://127.0.0.1"): string {
  if (environment === "native") return NativeResourcePolicy;
  return PreviewResourcePolicy.replaceAll("{origin}", origin);
}

export const previewHostPolicy = (origin: string) => PreviewHostPolicy.replaceAll("{origin}", origin);

/** The browser permission features a declared capability delegates to the app's frame.
 * `speech` is a host service, not a browser permission, so it delegates nothing. */
const deviceFeatures = ["camera", "microphone"] as const satisfies readonly Capability[];
export const capabilityFeatures = (capabilities: readonly Capability[]) =>
  deviceFeatures.filter(feature => capabilities.includes(feature));
/** A `Permissions-Policy` header that admits only the declared devices, to the app's origin. */
export const capabilityPermissionsPolicy = (capabilities: readonly Capability[]) =>
  deviceFeatures.map(feature => `${feature}=${capabilities.includes(feature) ? "(self)" : "()"}`).join(", ");
