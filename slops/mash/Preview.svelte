<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureMode, CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import Capture from "./Export.svelte";
  let props: CaptureProps<typeof schema> & { mode: CaptureMode } = $props();
</script>

<Capture {...props} preview />
