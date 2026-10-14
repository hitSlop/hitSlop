<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import Sun from "@lucide/svelte/icons/sun";
  import Moon from "@lucide/svelte/icons/moon";
  import Check from "@lucide/svelte/icons/check";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";

  let { snapshot }: CaptureProps<typeof schema> = $props();
  const morningDone = $derived(snapshot.morningDone);
  const eveningDone = $derived(snapshot.eveningDone);
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
