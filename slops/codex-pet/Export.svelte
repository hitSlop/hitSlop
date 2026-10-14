<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import Portrait from "./Portrait.svelte";
  let { snapshot }: CaptureProps<typeof schema> = $props();
</script>

<Portrait {snapshot} label="Exported Codex Pet" />
