<script lang="ts">
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  let { snapshot }: CaptureProps<typeof schema> = $props();
</script>
<main class="icon" data-capture-mode="icon"><strong>{snapshot.hits}</strong><span>{snapshot.title}</span></main>
