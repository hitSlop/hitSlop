<script lang="ts">
import { useDocument } from "@hitslop/document/svelte";
import schema from "./schema";
import Case from "./Case.svelte";
import ClickWheel from "./ClickWheel.svelte";
import CoverFlow from "./CoverFlow.svelte";
import StickerLayer from "./StickerLayer.svelte";

// The same body, with the current video centered in Cover Flow.
const doc = useDocument(schema);
const videos = $derived(doc.current.videos);
const cursor = $derived(Math.max(0, videos.findIndex((video) => video.$id === doc.current.nowPlayingId)));
</script>

{#snippet stickers()}
  <StickerLayer stickers={doc.current.stickers} />
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
