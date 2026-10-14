<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import Lamp from "./StillLamp.svelte";
  let { snapshot }: CaptureProps<typeof schema> = $props();
</script>

<div class="lamp-icon"><Lamp on={snapshot.on} /></div>
