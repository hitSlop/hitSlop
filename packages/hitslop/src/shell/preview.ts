/** The development host's transport, when `slop dev` serves this page. Read once as the
 * shell loads, before any app code runs, and removed from the page: an app cannot later
 * install one to redirect its own bridge. Native pages never have it. */
import "./globals";
import type { PagePush } from "../wire/page";
export type PreviewHost = {
  uiURL: string;
  attachmentURL(id: string): string;
  request(request: unknown): Promise<unknown>;
  onPush(receiver: (pushes: PagePush[]) => void): void;
};
export const preview: { host?: PreviewHost } = { host: globalThis.__slopPreview };
delete globalThis.__slopPreview;
