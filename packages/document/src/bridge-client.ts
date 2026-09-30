/** The sole native message endpoint. Each caller validates its own reply contract. */
export function postToHost(request: unknown): Promise<any> {
  return (globalThis as any).webkit.messageHandlers.hitslop.postMessage(request);
}
