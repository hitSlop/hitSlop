import { DocumentLineage } from "../schema/constants";
import { firebaseConfig, type FirebaseConfig } from "./config";

const database = "pop-browser-identity";
const store = "identities";
const attention =
  "This guest session can no longer connect. Your browser copy is kept. Export it, or start a new guest session to rejoin using the invitation.";
export class GuestIdentityAttention extends Error {
  constructor() {
    super(attention);
    this.name = "GuestIdentityAttention";
  }
}
interface Credential {
  uid: string;
  refreshToken: string;
}
interface Session extends Credential {
  token: string;
  expires: number;
}
function validCredential(value: unknown): value is Credential {
  const item = value as Credential | undefined;
  return (
    typeof item?.uid === "string" &&
    item.uid.length > 0 &&
    item.uid.length <= 128 &&
    typeof item.refreshToken === "string" &&
    item.refreshToken.length > 0 &&
    item.refreshToken.length <= 16_384
  );
}
async function credentials<T>(operation: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const db = await new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open(database, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(store);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(new Error("Guest session storage is unavailable"));
  });
  try {
    return await new Promise<T>((resolve, reject) => {
      const tx = db.transaction(store, "readwrite", { durability: "strict" });
      const request = operation(tx.objectStore(store));
      tx.oncomplete = () => resolve(request.result);
      tx.onerror = tx.onabort = () => reject(new Error("Guest session could not be saved"));
    });
  } finally {
    db.close();
  }
}
async function exchange(
  url: string,
  body: BodyInit,
  contentType: string,
): Promise<Record<string, unknown>> {
  const response = await fetch(url, {
    method: "POST",
    body,
    headers: { "Content-Type": contentType },
    credentials: "omit",
    signal: AbortSignal.timeout(30_000),
  });
  // Google's response is small. Read incrementally so a malformed upstream reply is bounded.
  if (!response.body) throw new Error("Guest session service is unavailable");
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      size += value.length;
      if (size > 65_536) throw new Error("Guest session reply is too large");
      chunks.push(value);
    }
  } finally {
    await reader.cancel().catch(() => {});
  }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  let result: Record<string, unknown>;
  try {
    result = JSON.parse(new TextDecoder().decode(bytes));
  } catch {
    throw new Error("Guest session service returned an invalid reply");
  }
  if (!result || typeof result !== "object" || Array.isArray(result))
    throw new Error("Guest session service returned an invalid reply");
  if (!response.ok) {
    const message = (result.error as { message?: unknown } | undefined)?.message;
    if (
      typeof message === "string" &&
      [
        "INVALID_REFRESH_TOKEN",
        "TOKEN_EXPIRED",
        "USER_DISABLED",
        "USER_NOT_FOUND",
        "INVALID_GRANT",
        "CREDENTIAL_TOO_OLD_LOGIN_AGAIN",
      ].includes(message)
    )
      throw new GuestIdentityAttention();
    throw new Error("Guest session service is unavailable; retry later");
  }
  return result;
}
/** Credentials live only in the trusted host's storage. Neither refresh nor ID tokens
 * are sent to an authored frame. Firebase REST: firebase.google.com/docs/reference/rest/auth */
export class GuestIdentity {
  private session: Session | undefined;
  private uid: string | undefined;
  constructor(private readonly configuration: () => Promise<FirebaseConfig> = firebaseConfig) {}
  async token(): Promise<string> {
    const config = await this.configuration();
    const key = `${DocumentLineage}:${config.projectId}`;
    return navigator.locks.request(`pop-identity:${key}`, async () => {
      const saved = await credentials<unknown>((store) => store.get(key));
      if (saved !== undefined && !validCredential(saved)) throw new GuestIdentityAttention();
      if (this.uid !== undefined && saved?.uid !== this.uid) throw new GuestIdentityAttention();
      if (
        saved &&
        this.session?.refreshToken === saved.refreshToken &&
        this.session.expires > Date.now() + 60_000
      )
        return this.session.token;
      const result = saved
        ? await exchange(
            `https://securetoken.googleapis.com/v1/token?key=${encodeURIComponent(config.apiKey)}`,
            new URLSearchParams({ grant_type: "refresh_token", refresh_token: saved.refreshToken }),
            "application/x-www-form-urlencoded",
          )
        : await exchange(
            `https://identitytoolkit.googleapis.com/v1/accounts:signUp?key=${encodeURIComponent(config.apiKey)}`,
            JSON.stringify({ returnSecureToken: true }),
            "application/json",
          );
      const session = {
        uid: (saved ? result.user_id : result.localId) as string,
        refreshToken: (saved ? result.refresh_token : result.refreshToken) as string,
        token: (saved ? result.id_token : result.idToken) as string,
        expires: Date.now() + Number(saved ? result.expires_in : result.expiresIn) * 1000,
      };
      if (
        !validCredential(session) ||
        typeof session.token !== "string" ||
        !session.token.length ||
        session.token.length > 16_384 ||
        !Number.isFinite(session.expires) ||
        session.expires <= Date.now() ||
        (saved && saved.uid !== session.uid)
      )
        throw new GuestIdentityAttention();
      await credentials((store) =>
        store.put({ uid: session.uid, refreshToken: session.refreshToken }, key),
      );
      this.uid = session.uid;
      this.session = session;
      return session.token;
    });
  }
  /** Explicit host action only, after the document owner has flushed accepted edits. */
  async reset(): Promise<void> {
    const config = await this.configuration();
    const key = `${DocumentLineage}:${config.projectId}`;
    await navigator.locks.request(`pop-identity:${key}`, async () => {
      await credentials((store) => store.delete(key));
      this.uid = undefined;
      this.session = undefined;
    });
    await this.token();
  }
}
