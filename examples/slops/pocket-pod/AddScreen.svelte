<script lang="ts">
import { onMount } from "svelte";
import TitleBar from "./TitleBar.svelte";

let { text = $bindable(), busy, playing, paused, held, onsubmit, onescape }: {
  text: string;
  busy: boolean;
  playing?: boolean;
  paused?: boolean;
  held?: boolean;
  onsubmit: () => void;
  onescape: () => void;
} = $props();

let field: HTMLInputElement;
onMount(() => field.focus());
</script>

<section class="pod-screen pod-add" aria-label="Add Video">
  <TitleBar title="Add Video" {playing} {paused} {held} />
  <form
    class="pod-add-body"
    novalidate
    onsubmit={(event) => { event.preventDefault(); onsubmit(); }}
  >
    <label for="pod-add-link">Paste a YouTube link</label>
    <input
      id="pod-add-link"
      bind:this={field}
      bind:value={text}
      type="text"
      inputmode="url"
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
      placeholder="youtube.com/watch?v=…"
      disabled={busy}
      onkeydown={(event) => { if (event.key === "Escape") { event.preventDefault(); onescape(); } }}
    />
    <p class="pod-add-hint">
      {#if busy}Looking up video…{:else}Press ● to add. You can also paste a link on any screen.{/if}
    </p>
  </form>
</section>
