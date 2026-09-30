<script lang="ts">
import { useDocument } from "@hitslop/document/svelte";
import schema from "./schema";
import { pageView } from "./model";
const doc = useDocument(schema);
const { wordsCount, page3Done } = $derived(pageView(doc.current));
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
