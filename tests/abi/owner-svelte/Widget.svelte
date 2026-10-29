<script lang="ts">
  import "./styles.css";
  import type { CaptureProps, WidgetProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  let { snapshot, family, width, height }: CaptureProps<typeof schema> & WidgetProps = $props();
</script>
<main data-widget-family={family} style:width={`${width}px`} style:height={`${height}px`}><div class="accent-patch"></div><h1>{snapshot.title}</h1><p>{snapshot.rows.length} rows / {snapshot.hits} hits</p><p class="cjk">中文 😀</p></main>
