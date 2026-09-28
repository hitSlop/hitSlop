// Socket request/reply types shared by the session and native helper.
export type {
  SocketReply as Reply,
  SocketReplyCode as ReplyCode,
} from "@hitslop/schema/socket";

import type { SocketRequest } from "@hitslop/schema/socket";
/** Native handles export before dispatching requests to the JS session. */
export type Request = Exclude<SocketRequest, { method: "export" }>;
