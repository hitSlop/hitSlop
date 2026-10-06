/** The app's resource policy. Only local resource origins and development HMR differ. */
export function appContentSecurityPolicy(environment: "native" | "browser"): string {
  const local = environment === "native" ? "slop:" : "'self'";
  return [
    "default-src 'none'",
    `script-src ${local} 'wasm-unsafe-eval'`,
    `connect-src ${local}${environment === "browser" ? " ws://127.0.0.1:*" : ""} https: blob:`,
    `media-src ${local} https: blob:`,
    "frame-src https:",
    `style-src ${local} 'unsafe-inline'`,
    `img-src ${local} data: https: blob:`,
    `font-src ${local} data:`,
  ].join("; ");
}
