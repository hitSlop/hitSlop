import type { Binding } from "../abi";
import type { TextHandle } from "../handle-types";
import { current } from "./context";

/** Binds a native input to host-owned text merging, caret and IME behavior. */
export function bindText(element: HTMLInputElement | HTMLTextAreaElement, handle: TextHandle): Binding<TextHandle> {
  return current().bind.text(element, handle);
}
