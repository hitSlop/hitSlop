<script lang="ts">
  import doc from "./schema";
  import { householdView } from "./chores";
  import Wheel from "./Wheel.svelte";
  const { totals } = $derived(householdView(doc.current));
</script>

<div class="icon-surface" aria-hidden="true">
  <div class="icon-wheel"><Wheel people={doc.current.people} {totals} size={300} /></div>
  <span class="icon-mag a"></span><span class="icon-mag b"></span><span class="icon-mag c"></span>
</div>
