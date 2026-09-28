<script lang="ts">
  import { useDocument } from "@hitslop/document/svelte";
  import Sun from "@lucide/svelte/icons/sun";
  import Moon from "@lucide/svelte/icons/moon";
  import Check from "@lucide/svelte/icons/check";
  import schema from "./schema";

  const doc = useDocument(schema);
  const morningDone = $derived(doc.current.morningDone);
  const eveningDone = $derived(doc.current.eveningDone);
</script>
<div class={"fmj-iconSurface"} aria-hidden="true">
  <article class={"fmj-iconPlate"}>
    <div class={"fmj-iconBookmarks"}>
      <span class={"fmj-iconMorning fmj-iconBookmark"}><Sun size={30} strokeWidth={1.5} /></span>
      <span class={"fmj-iconEvening fmj-iconBookmark"}><Moon size={30} strokeWidth={1.5} /></span>
    </div>
    <span class={"fmj-iconTitle"}>Five<br />Minute<br />Journal</span>
    <div class={"fmj-iconEntry"}><Sun size={22} /><span class={"fmj-iconRule"}></span>{#if morningDone}<Check size={22} />{/if}</div>
    <div class={"fmj-iconEntry"}><Moon size={22} /><span class={"fmj-iconRule"}></span>{#if eveningDone}<Check size={22} />{/if}</div>
  </article>
</div>
