import type { ViewCaptureKind } from "../../wire/page.generated";
import { deepFreeze } from "../descriptor";
import type { Definition, ObjectNode, Value } from "../schema";

/** The static view being rendered, passed to Preview, Export and Icon as `mode`. */
export type CaptureMode = ViewCaptureKind;

/**
 * What every static view receives: Preview, Export and Icon (plus `mode: CaptureMode`) and
 * Widget (plus its `WidgetProps`). They render the saved document from these props alone:
 *
 * ```svelte
 * <script lang="ts">
 *   import type { CaptureProps } from "@hitslop/tinys/svelte";
 *   import type schema from "./schema";
 *   let { snapshot, attachmentURL }: CaptureProps<typeof schema> = $props();
 * </script>
 * ```
 *
 * Import the schema with `import type`: a static view needs only its type, so the
 * definition stays out of the capture.
 */
export type CaptureProps<D extends Definition<ObjectNode>> = {
  /** The saved document, frozen all the way down. */
  readonly snapshot: Value<D["descriptor"]>;
  /** The URL of an image the document stores, by attachment ID. Throws when the ID is missing
   * or is not a supported image. */
  attachmentURL(id: string): string;
};

/** One render request's props: its snapshot frozen in place, and its images by ID. */
export function captureProps(
  snapshot: unknown,
  attachments: Record<string, string>,
): CaptureProps<Definition<ObjectNode>> {
  const images = Object.freeze({ ...attachments });
  return {
    snapshot: deepFreeze(snapshot) as Value<ObjectNode>,
    attachmentURL(id) {
      if (
        typeof id !== "string" ||
        !Object.hasOwn(images, id) ||
        typeof images[id] !== "string" ||
        !["image/png", "image/jpeg", "image/gif", "image/webp"].includes(images[id])
      )
        throw new Error(`Capture attachment ${id} is missing or is not a supported image`);
      return `tiny-attachment:${id}`;
    },
  };
}
