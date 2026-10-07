import type { EngineRequest, EngineSuccess, Failure } from "./engine.generated";
export type { EngineRequest, EngineReply, EngineSuccess } from "./engine.generated";
export type EngineMethod = EngineRequest["method"];
export type EngineRequestFor<M extends EngineMethod> = Extract<EngineRequest, { method: M }>;
export type EngineReplyFor<M extends EngineMethod> = Extract<EngineSuccess, { method: M }> | Failure;
