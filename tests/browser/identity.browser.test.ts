import { expect, test } from "bun:test";
import { chromium } from "playwright";

// Real trusted-origin IndexedDB and Web Locks, with fake Firebase REST responses.
// No Firebase users or remote configuration are changed by these tests.
test("guest identities persist across tabs and require explicit recovery", async () => {
  const bundle = await Bun.build({ entrypoints: ["packages/pop/src/sharing/identity.ts"], target: "browser" });
  expect(bundle.success).toBe(true);
  const source = await bundle.outputs[0]!.text();
  const server = Bun.serve({ port: 0, fetch(request) {
    return new Response(new URL(request.url).pathname === "/identity.js" ? source :
      '<script type="module">import {GuestIdentity} from "/identity.js";window.identity=new GuestIdentity(async()=>({apiKey:"fake",projectId:"test-project"}));</script>',
      { headers: { "Content-Type": new URL(request.url).pathname === "/identity.js" ? "application/javascript" : "text/html" } });
  } });
  const browser = await chromium.launch({ channel: "chrome" });
  try {
    const context = await browser.newContext();
    let creates = 0, refreshes = 0, refuse = false;
    await context.route("https://identitytoolkit.googleapis.com/**", route => {
      creates++;
      return route.fulfill({ json: { localId: `uid-${creates}`, refreshToken: `refresh-${creates}`, idToken: `token-${creates}`, expiresIn: "3600" } });
    });
    await context.route("https://securetoken.googleapis.com/**", route => {
      refreshes++;
      const id = new URLSearchParams(route.request().postData()!).get("refresh_token")!.split("-")[1];
      return route.fulfill(refuse ? { status: 400, json: { error: { message: "INVALID_REFRESH_TOKEN" } } } :
        { json: { user_id: `uid-${id}`, refresh_token: `refresh-${id}`, id_token: `renewed-${id}`, expires_in: "3600" } });
    });
    const one = await context.newPage(), two = await context.newPage();
    await Promise.all([one.goto(server.url.href), two.goto(server.url.href)]);
    await Promise.all([one.waitForFunction(() => !!(window as any).identity), two.waitForFunction(() => !!(window as any).identity)]);
    const token = (page: typeof one) => page.evaluate(() => (window as any).identity.token());
    expect((await Promise.all([token(one), token(two)])).sort()).toEqual(["renewed-1", "token-1"]);
    expect(creates).toBe(1);
    expect(refreshes).toBe(1);
    await one.evaluate(() => { const now = Date.now(); Date.now = () => now + 3_600_000; });
    expect(await token(one)).toBe("renewed-1");
    expect(refreshes).toBe(2);
    await one.reload(); await one.waitForFunction(() => !!(window as any).identity);
    expect(await token(one)).toBe("renewed-1");
    refuse = true;
    await one.reload(); await one.waitForFunction(() => !!(window as any).identity);
    expect(await one.evaluate(() => (window as any).identity.token().then(() => "unexpected", (e: Error) => e.name))).toBe("GuestIdentityAttention");
    expect(creates).toBe(1);
    // An ordinary retry never silently discards the retained credential.
    expect(await one.evaluate(() => (window as any).identity.token().then(() => "unexpected", (e: Error) => e.name))).toBe("GuestIdentityAttention");
    expect(creates).toBe(1);
    refuse = false;
    await one.evaluate(() => (window as any).identity.reset());
    expect(creates).toBe(2);
    // A different tab cannot silently cross identities when another tab explicitly resets.
    expect(await two.evaluate(() => (window as any).identity.token().then(() => "unexpected", (e: Error) => e.name))).toBe("GuestIdentityAttention");
    await context.close();
  } finally { await browser.close(); server.stop(true); }
}, 30_000);
