<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import CaptureContent from "./CaptureContent.svelte";
  let { snapshot, attachmentURL }: CaptureProps<typeof schema> = $props();
</script>
<main data-capture-mode="preview"><h1>{snapshot.title}</h1><CaptureContent {snapshot} {attachmentURL} /></main>
