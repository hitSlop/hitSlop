export async function call(method: string, args: Record<string, unknown> = {}) {
  const text = await (globalThis as any).webkit.messageHandlers.spike.postMessage(
    JSON.stringify({ method, ...args }),
  );
  return JSON.parse(text);
}
