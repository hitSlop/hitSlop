<script lang="ts">
import { Dialog, Button } from "bits-ui";
import { overlayTransition, surfaceTransition } from "@hitslop/pop/svelte";
import type { Snippet } from "svelte";
let { open, title, description, onclose, children }: {
  open: boolean;
  title: string;
  description: string;
  onclose: () => void;
  children: Snippet;
} = $props();
let content = $state<HTMLDivElement>();
</script>

<Dialog.Root {open} onOpenChange={(value) => { if (!value) onclose(); }}>
  <Dialog.Portal>
    <Dialog.Overlay forceMount>
      {#snippet child({ props })}
        {#if open}<div {...props} class="pod-sheet-overlay" transition:overlayTransition></div>{/if}
      {/snippet}
    </Dialog.Overlay>
    <Dialog.Content forceMount onOpenAutoFocus={(event) => { event.preventDefault(); content?.focus(); }}>
      {#snippet child({ props })}
        {#if open}
          <div {...props} bind:this={content} class="pod-sheet" tabindex="-1" transition:surfaceTransition={{ kind: "sheet" }}>
            <header class="pod-sheet-heading">
              <div><Dialog.Title>{title}</Dialog.Title><Dialog.Description>{description}</Dialog.Description></div>
              <Button.Root class="pod-sheet-close" aria-label="Close" onclick={onclose}>×</Button.Root>
            </header>
            <div class="pod-sheet-body">{@render children()}</div>
          </div>
        {/if}
      {/snippet}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
