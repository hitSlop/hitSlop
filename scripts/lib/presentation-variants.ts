import type { AppDeclaration } from "../../packages/tinys/src/sdk/tiny";

/** Source variants consumed by the native fixture builder. */
export const presentationVariants = {
  fixed: { kind: "standard", width: 320, height: 320, resizable: false },
  evenodd: {
    kind: "standard",
    width: 320,
    height: 320,
    background: "transparent",
    shape: {
      path: "M16 0 h288 a16 16 0 0 1 16 16 v288 c0 9 -7 16 -16 16 H16 Q0 320 0 304 V16 A16 16 0 0 1 16 0 Z M80 80 h160 v160 h-160 z",
      viewBox: [320, 320],
      fillRule: "evenodd",
    },
  },
  radii: { kind: "standard", width: 320, height: 320, shape: "12px 24px / 50%" },
  lockAspect: { kind: "standard", width: 360, height: 240, lockAspect: true },
} as const satisfies Record<string, AppDeclaration["window"]>;

/** The conformance app declares no services (the control); this variant declares every capability. */
export const declaredCapabilities = {
  capabilities: ["camera", "microphone", "speech"],
} as const satisfies Pick<AppDeclaration, "capabilities">;
