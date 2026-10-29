<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import type { CaptureMode, CaptureProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  import Capture from "./Export.svelte";
  import PreviewNote from "../_shared/PreviewNote.svelte";
  let props: CaptureProps<typeof schema> & { mode: CaptureMode } = $props();
</script>

<Capture {...props} preview />
<PreviewNote>A quick look. Open the document for the full view.</PreviewNote>
