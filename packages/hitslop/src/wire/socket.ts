import type { EngineSuccess, Failure } from "./engine.generated";
import type { SocketRequest } from "./socket.generated";
export type { SocketRequest } from "./socket.generated";
export type SocketMethod = SocketRequest["method"];
// Socket operations are also engine operations, with exactly the same replies.
export type SocketSuccess = Extract<EngineSuccess, { method: SocketMethod }>;
export type SocketFailure = Failure;
export type SocketReply = SocketSuccess | SocketFailure;
