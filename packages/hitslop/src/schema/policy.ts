import { NativeResourcePolicy } from "../wire/constants.generated";
/** Vite serves every source module under /__app__; attachment URLs never match it. */
export function appContentSecurityPolicy(environment: "native" | "browser", origin = "http://127.0.0.1"): string {
  if (environment === "native") return NativeResourcePolicy;
  return [
    "default-src 'none'",
    `script-src ${origin}/__shell__/ ${origin}/__app__/ ${origin}/__preview__/native.js 'wasm-unsafe-eval'`,
    `connect-src 'self' ws://127.0.0.1:* https: blob:`,
    "media-src 'self' https: blob:", "frame-src https:",
    `style-src ${origin}/__app__/ 'unsafe-inline'`,
    "img-src 'self' data: https: blob:", "font-src 'self' data:",
  ].join("; ");
}
