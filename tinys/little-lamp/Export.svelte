<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import type { CaptureProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  import Lamp from "./StillLamp.svelte";
  let { snapshot }: CaptureProps<typeof schema> = $props();
</script>

<div class="lamp-export" role="img" aria-label={snapshot.on ? "Little Lamp, awake" : "Little Lamp, asleep"}>
  <Lamp on={snapshot.on} />
</div>
