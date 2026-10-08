import type { PageRequest as Request, PageSuccess } from "./page.generated";
export type { HostRequest, HostCaptureResult, PagePush } from "./page.generated";
export type { Failure as PageFailure } from "./engine.generated";
export type PageMethod = Request["method"];
export type PageRequest<M extends PageMethod = PageMethod> = Extract<Request, { method: M }>;
export type PageResult<M extends PageMethod> = Omit<Extract<PageSuccess, { method: M }>, "ok" | "method">;
