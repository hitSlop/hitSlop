import type { HostEnvironment } from "../wire/page.generated";

/** Layout simulation only: these presets grant no native services or permissions. */
export const environmentPresets = {
  browser: { platform: "unknown", device: "unknown" },
  iphone: { platform: "ios", device: "phone", width: 390, height: 844 },
  ipad: { platform: "ios", device: "tablet", width: 768, height: 1024 },
  mac: { platform: "macos", device: "desktop" },
} as const satisfies Record<string, HostEnvironment & { width?: number; height?: number }>;
export type EnvironmentPreset = keyof typeof environmentPresets;
export function isEnvironmentPreset(value: unknown): value is EnvironmentPreset {
  return typeof value === "string" && Object.hasOwn(environmentPresets, value);
}
