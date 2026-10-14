<script lang="ts">
  import type { CaptureMode, CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import CaptureContent from "./CaptureContent.svelte";
  let { snapshot, attachmentURL, mode }: CaptureProps<typeof schema> & { mode: CaptureMode } = $props();
</script>
<main data-capture-mode={mode}><h1>{snapshot.title}</h1><CaptureContent {snapshot} {attachmentURL} /></main>
