<script lang="ts">
  import { Dialog } from "bits-ui";
  import { surfaceTransition, overlayTransition } from "hitslop/svelte";
  let open = $state(false);
  let value = $state("");
  let opener = $state<HTMLButtonElement | null>(null);
</script>
<Dialog.Root bind:open>
  <Dialog.Trigger bind:ref={opener}>Open surface</Dialog.Trigger>
  <Dialog.Portal>
    <Dialog.Overlay forceMount>
      {#snippet child({ props, open })}{#if open}
        <div {...props} class="overlay" transition:overlayTransition></div>
      {/if}{/snippet}
    </Dialog.Overlay>
    <Dialog.Content forceMount onCloseAutoFocus={event => {
      event.preventDefault();
      opener?.focus({ preventScroll: true });
    }}>
      {#snippet child({ props, open })}{#if open}
        <div {...props} class="surface" transition:surfaceTransition>
          <Dialog.Title>Interaction surface</Dialog.Title>
          <Dialog.Description>A small SDK interaction fixture.</Dialog.Description>
          <input aria-label="Surface draft" bind:value />
          <Dialog.Close>Close surface</Dialog.Close>
        </div>
      {/if}{/snippet}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
<style>
  .overlay { position: fixed; inset: 0; background: #0003; }
  .surface { position: fixed; left: 20px; top: 20px; padding: 20px; background: white; }
</style>
