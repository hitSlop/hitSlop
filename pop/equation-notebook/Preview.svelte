<script lang="ts">
  import "@hitslop/pop/base.css";
  import "./mathlive-static.css";
  import "./styles.css";
  import type { CaptureMode, CaptureProps } from "@hitslop/pop/svelte";
  import type schema from "./schema";
  import Capture from "./Export.svelte";
  import PreviewNote from "../_shared/PreviewNote.svelte";
  let props: CaptureProps<typeof schema> & { mode: CaptureMode } = $props();
</script>

<Capture {...props} preview />
<PreviewNote>A quick look. Open the document for the full view.</PreviewNote>
