/** Host requests to the copy worker and page. Page-owned IDs pass through separately. */
export class PendingRequests {
  private readonly pending = new Map<string, { resolve: (data: any) => void; reject: (error: Error) => void }>();

  start(send: (id: string) => void, timeout?: { after: number; message: string }): Promise<any> {
    const id = crypto.randomUUID();
    return new Promise((resolve, reject) => {
      const timer = timeout && setTimeout(() => this.settle(id, undefined, new Error(timeout.message)), timeout.after);
      this.pending.set(id, {
        resolve: data => { clearTimeout(timer); resolve(data); },
        reject: error => { clearTimeout(timer); reject(error); },
      });
      try { send(id); }
      catch (error) { this.settle(id, undefined, error instanceof Error ? error : new Error(String(error))); }
    });
  }

  settle(id: string, data: unknown, error?: Error): boolean {
    const pending = this.pending.get(id);
    if (!pending) return false;
    this.pending.delete(id);
    if (error) pending.reject(error); else pending.resolve(data);
    return true;
  }

  fail(error: Error) {
    for (const id of this.pending.keys()) this.settle(id, undefined, error);
  }
}
