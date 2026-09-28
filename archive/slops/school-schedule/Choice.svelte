<script lang="ts">
  import { Select } from "bits-ui";
  import Check from "@lucide/svelte/icons/check";
    let {
    value = $bindable(""),
    label,
    items,
  }: {
    value?: string;
    label: string;
    items: { value: string; label: string }[];
  } = $props();
</script>

<div class={"sch-formLabel"}>
  <span>{label}</span><Select.Root type="single" bind:value
    ><Select.Trigger class={"sch-input"} aria-label={label}
      >{items.find((i) => i.value === value)?.label || "Choose…"}<span
        aria-hidden="true">⌄</span
      ></Select.Trigger
    ><Select.Portal
      ><Select.Content
        class={"sch-selectContent"}
        sideOffset={5}
        collisionPadding={8}
        ><Select.Viewport
          >{#each items as item}<Select.Item
              value={item.value}
              label={item.label}
              class={"sch-selectItem"}
              >{#snippet children({ selected })}{item.label}{#if selected}<Check
                    size={14}
                  />{/if}{/snippet}</Select.Item
            >{/each}</Select.Viewport
        ></Select.Content
      ></Select.Portal
    ></Select.Root
  >
</div>
