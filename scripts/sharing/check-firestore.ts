/** Real emulator boundary: clients can discover their own catalog but cannot publish it. */
import assert from "node:assert/strict";

const project = "demo-pop-catalog";
const firestore = process.env.FIRESTORE_EMULATOR_HOST;
const auth = process.env.FIREBASE_AUTH_EMULATOR_HOST;
if (!firestore || !auth) throw new Error("Run with Firebase Auth and Firestore emulators");
const base = `http://${firestore}/v1/projects/${project}/databases/(default)/documents`;
async function account() {
  const response = await fetch(`http://${auth}/identitytoolkit.googleapis.com/v1/accounts:signUp?key=local`, {
    method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ returnSecureToken: true }),
  });
  assert.equal(response.status, 200);
  return await response.json() as { localId: string; idToken: string };
}
const owner = await account(), other = await account();
const path = `/users/${owner.localId}/documents/00000000-0000-4000-8000-000000000001`;
const fields = {
  documentId: { stringValue: "00000000-0000-4000-8000-000000000001" },
  title: { stringValue: "Shared document" },
  deleted: { booleanValue: false },
  catalogRevision: { integerValue: "1" },
  modifiedAt: { timestampValue: "2026-01-01T00:00:00Z" },
};
const request = (suffix: string, token: string, method = "GET", body?: unknown) => fetch(base + suffix, {
  method, headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
  ...(body === undefined ? {} : { body: JSON.stringify(body) }),
});
// The emulator's owner credential represents service/IAM publication, not a client bypass.
assert.equal((await request(path, "owner", "PATCH", { fields })).status, 200);
assert.equal((await request(path, owner.idToken)).status, 200);
assert.equal((await request(path, other.idToken)).status, 403);
assert.equal((await fetch(base + path)).status, 403);
assert.equal((await request(path, owner.idToken, "PATCH", { fields })).status, 403);
assert.equal((await request(path, owner.idToken, "DELETE")).status, 403);
assert.equal((await request(path + "-forged", owner.idToken, "PATCH", { fields })).status, 403);
assert.equal((await request(`/users/${owner.localId}/documents`, owner.idToken)).status, 200);
assert.equal((await request(`/users/${owner.localId}/documents`, other.idToken)).status, 403);
assert.equal((await request("/templates/private", owner.idToken)).status, 403);
console.log("Firestore catalog isolation and backend-only publication passed");
