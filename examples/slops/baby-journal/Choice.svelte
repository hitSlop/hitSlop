<script lang="ts">
  import { Select } from "bits-ui";
  let { value = $bindable(""), options, label }: {
    value?: string; options: { value: string; label: string }[]; label: string;
  } = $props();
</script>
<Select.Root type="single" bind:value>
  <Select.Trigger class="bj-select" aria-label={label}>
    <span>{options.find(option => option.value === value)?.label ?? "Choose…"}</span><span aria-hidden="true">⌄</span>
  </Select.Trigger>
  <Select.Portal>
    <Select.Content class="bj-options" sideOffset={6}>
      <Select.Viewport>
        {#each options as option (option.value)}
          <Select.Item class="bj-option" value={option.value} label={option.label}>{option.label}</Select.Item>
        {/each}
      </Select.Viewport>
    </Select.Content>
  </Select.Portal>
</Select.Root>
