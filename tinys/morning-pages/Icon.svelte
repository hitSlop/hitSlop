<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import type { CaptureProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  import { pageView } from "./model";
  let { snapshot }: CaptureProps<typeof schema> = $props();
  const { wordsCount, page3Done } = $derived(pageView(snapshot));
</script>

<div class={"mp-iconSurface"} aria-hidden="true">
  <div class={"mp-iconPad"}>
    <span class={"mp-iconMargin"}></span>
    <div class={"mp-iconLines"}>
      <span class={"mp-iconLine"}></span>
      <span class={"mp-iconLine"}></span>
      <span class={"mp-iconLine"}></span>
      <span class={"mp-iconLine"}></span>
      <span class={"mp-iconLine"}></span>
    </div>
    <span class={"mp-iconSeal"} data-complete={page3Done}>{page3Done ? "750" : String(wordsCount).padStart(3, "0")}</span>
  </div>
</div>
