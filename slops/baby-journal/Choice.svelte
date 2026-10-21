<script lang="ts">
  import { haptics, popoverTransition } from "hitslop/svelte";
  import { Select } from "bits-ui";
  let { value = $bindable(""), options, label }: {
    value?: string; options: { value: string; label: string }[]; label: string;
  } = $props();
</script>
<Select.Root type="single" bind:value onValueChange={() => haptics.selection()}>
  <Select.Trigger class="bj-select" aria-label={label}>
    <span>{options.find(option => option.value === value)?.label ?? "Choose…"}</span><span aria-hidden="true">⌄</span>
  </Select.Trigger>
  <Select.Portal>
    <Select.Content sideOffset={6} forceMount>
      {#snippet child({ wrapperProps, props, open })}{#if open}
      <div {...wrapperProps}><div {...props} class="bj-options" transition:popoverTransition>
      <Select.Viewport>
        {#each options as option (option.value)}
          <Select.Item class="bj-option" value={option.value} label={option.label}>{option.label}</Select.Item>
        {/each}
      </Select.Viewport>
    </div></div>{/if}{/snippet}</Select.Content>
  </Select.Portal>
</Select.Root>
