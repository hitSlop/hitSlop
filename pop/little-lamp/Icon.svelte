<script lang="ts">
  import "@hitslop/pop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "@hitslop/pop/svelte";
  import type schema from "./schema";
  import Lamp from "./StillLamp.svelte";
  let { snapshot }: CaptureProps<typeof schema> = $props();
</script>

<div class="lamp-icon"><Lamp on={snapshot.on} /></div>
