<script lang="ts">
import "@hitslop/pop/base.css";
import "./styles.css";
import type { CaptureProps } from "@hitslop/pop/svelte";
import type schema from "./schema";
import Case from "./Case.svelte";
import Background from "./Background.svelte";
import ClickWheel from "./ClickWheel.svelte";
import CoverFlow from "./CoverFlow.svelte";
import StickerLayer from "./StickerLayer.svelte";
import { podView } from "./pod";

let { snapshot, attachmentURL }: CaptureProps<typeof schema> = $props();

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
  <Background source={snapshot.background ? attachmentURL(snapshot.background.id) : undefined} fit={snapshot.backgroundFit} dim={snapshot.backgroundDim} />
  <Case {stickers} {screen} {tray} {wheel} />
</div>
