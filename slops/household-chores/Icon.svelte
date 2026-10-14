<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { householdView } from "./chores";
  import Wheel from "./Wheel.svelte";
  let { snapshot }: CaptureProps<typeof schema> = $props();
  const { totals } = $derived(householdView(snapshot));
</script>

<div class="icon-surface" aria-hidden="true">
  <div class="icon-wheel"><Wheel people={snapshot.people} {totals} size={300} /></div>
  <span class="icon-mag a"></span><span class="icon-mag b"></span><span class="icon-mag c"></span>
</div>
