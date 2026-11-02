import {
  NativeResourcePolicy,
  PreviewResourcePolicy,
  PreviewHostPolicy,
} from "../wire/constants.generated";
import type { Capability } from "../wire/app.generated";
/** Vite serves every source module under /__app__; attachment URLs never match it. */
export function appContentSecurityPolicy(
  environment: "native" | "browser",
  origin = "http://127.0.0.1",
): string {
  if (environment === "native") return NativeResourcePolicy;
  return PreviewResourcePolicy.replaceAll("{origin}", origin);
}

export const previewHostPolicy = (origin: string) =>
  PreviewHostPolicy.replaceAll("{origin}", origin);

/** The browser permission features a declared capability delegates to the app's frame.
 * `speech` is a host service, not a browser permission, so it delegates nothing. */
const browserFeatures = {
  camera: "camera",
  microphone: "microphone",
  speech: null,
} satisfies Record<Capability, string | null>;
const deviceFeatures = Object.entries(browserFeatures).filter(
  (entry): entry is [Capability, string] => entry[1] !== null,
);
export const capabilityFeatures = (capabilities: readonly Capability[]) =>
  deviceFeatures
    .filter(([capability]) => capabilities.includes(capability))
    .map(([, feature]) => feature);
/** A `Permissions-Policy` header that admits only the declared devices, to the app's origin. */
export const capabilityPermissionsPolicy = (capabilities: readonly Capability[]) =>
  deviceFeatures
    .map(
      ([capability, feature]) =>
        `${feature}=${capabilities.includes(capability) ? "(self)" : "()"}`,
    )
    .join(", ");
