import { SharingHost } from "../wire/constants.generated";
export const config = { host: SharingHost.origin, relay: SharingHost.origin };
export const frameOrigin = (copy: string) => `https://${copy}.${SharingHost.domain}`;

export interface FirebaseConfig {
  apiKey: string;
  projectId: string;
}
let loaded: Promise<FirebaseConfig> | undefined;
export function firebaseConfig(): Promise<FirebaseConfig> {
  return (loaded ??= loadFirebaseConfig().catch((error) => {
    loaded = undefined;
    throw error;
  }));
}
async function loadFirebaseConfig(): Promise<FirebaseConfig> {
  const response = await fetch(`${config.host}/auth/config`, {
    credentials: "omit",
    cache: "no-store",
    signal: AbortSignal.timeout(30_000),
  });
  if (!response.ok) throw new Error("Guest session configuration is unavailable; retry later");
  const value = (await response.json()) as Partial<FirebaseConfig>;
  if (
    !value ||
    typeof value !== "object" ||
    typeof value.apiKey !== "string" ||
    !value.apiKey ||
    value.apiKey.length > 256 ||
    typeof value.projectId !== "string" ||
    !/^[a-z0-9-]{1,128}$/.test(value.projectId)
  )
    throw new Error("Guest session configuration is invalid");
  return value as FirebaseConfig;
}
