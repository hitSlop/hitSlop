import { NativeResourcePolicy, PreviewResourcePolicy, PreviewHostPolicy } from "../wire/constants.generated";
/** Vite serves every source module under /__app__; attachment URLs never match it. */
export function appContentSecurityPolicy(environment: "native" | "browser", origin = "http://127.0.0.1"): string {
  if (environment === "native") return NativeResourcePolicy;
  return PreviewResourcePolicy.replaceAll("{origin}", origin);
}

export const previewHostPolicy = (origin: string) => PreviewHostPolicy.replaceAll("{origin}", origin);
