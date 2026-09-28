<script lang="ts">
  import Film from "@lucide/svelte/icons/film";
  import BriefcaseBusiness from "@lucide/svelte/icons/briefcase-business";
  import House from "@lucide/svelte/icons/house";
  import HeartPulse from "@lucide/svelte/icons/heart-pulse";
  import Repeat2 from "@lucide/svelte/icons/repeat-2";
  import type { Category } from "./schema";

  let { category }: { category: string } = $props();
  const marks: Partial<Record<Category, typeof Film>> = { Entertainment: Film, Work: BriefcaseBusiness, Home: House, Health: HeartPulse };
  const Mark = $derived(marks[category as Category] ?? Repeat2);
</script>

<span class="categoryMark" data-category={category} aria-hidden="true"><Mark size={19} strokeWidth={1.8} /></span>
