<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import Portrait from "./Portrait.svelte";
  let { snapshot }: CaptureProps<typeof schema> = $props();
</script>

<Portrait {snapshot} />
