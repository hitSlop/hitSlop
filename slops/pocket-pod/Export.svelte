<script lang="ts">
import "hitslop/base.css";
import "./styles.css";
import type { CaptureProps } from "hitslop/svelte";
import type schema from "./schema";
import Case from "./Case.svelte";
import ClickWheel from "./ClickWheel.svelte";
import CoverFlow from "./CoverFlow.svelte";
import StickerLayer from "./StickerLayer.svelte";
import { podView } from "./pod";

let { snapshot }: CaptureProps<typeof schema> = $props();

// The same body, with the current video centered in Cover Flow.
const { videos, currentIndex: cursor } = $derived(podView(snapshot));
</script>

{#snippet stickers()}
  <StickerLayer stickers={snapshot.stickers} />
{/snippet}

{#snippet screen()}
  <CoverFlow {videos} {cursor} onpick={() => {}} />
{/snippet}

{#snippet tray()}
  <span class="pod-deboss">POCKET POD</span>
{/snippet}

{#snippet wheel()}
  <ClickWheel held={false} ondispatch={() => {}} />
{/snippet}

<div class="pod-surface pod-export">
  <Case {stickers} {screen} {tray} {wheel} />
</div>
