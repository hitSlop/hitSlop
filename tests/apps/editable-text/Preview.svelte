<script lang="ts">
  import type { CaptureProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  let { snapshot }: CaptureProps<typeof schema> = $props();
</script>
<main><p>{snapshot.first}</p><p>{snapshot.second}</p></main>
