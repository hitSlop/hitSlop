<script lang="ts">
import { untrack } from "svelte";
import { Spring, prefersReducedMotion } from "svelte/motion";
import TitleBar from "./TitleBar.svelte";
import { youtube } from "@hitslop/document/embed";
import type { Video } from "./schema";

let { videos, cursor, playing, paused, held, onpick }: {
  videos: readonly Video[];
  cursor: number;
  playing?: boolean;
  paused?: boolean;
  held?: boolean;
  onpick: (index: number) => void;
} = $props();

const position = new Spring(untrack(() => cursor), { stiffness: 0.12, damping: 0.78 });
$effect(() => {
  void position.set(cursor, prefersReducedMotion.current ? { instant: true } : undefined);
});

const clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(high, value));
const selected = $derived(videos[cursor]);
const nearby = $derived(
  videos.flatMap((video, index) => (Math.abs(index - position.current) < 5 ? [{ video, index }] : [])),
);

function pose(index: number): string {
  const offset = index - position.current;
  const near = clamp(offset, -1, 1);
  const extra = offset - near;
  const x = near * 62 + extra * 28;
  const z = -Math.abs(near) * 74 - Math.abs(extra) * 10;
  return `translateX(${x}px) translateZ(${z}px) rotateY(${-near * 64}deg)`;
}
</script>

<section class="pod-screen pod-flow" aria-label="Cover Flow">
  <TitleBar title="Cover Flow" {playing} {paused} {held} />
  {#if videos.length}
    <div class="pod-flow-stage" role="listbox" aria-label="Videos">
      {#each nearby as { video, index } (video.$id)}
        <button
          type="button"
          class="pod-flow-cover"
          role="option"
          aria-selected={index === cursor}
          aria-label={video.title}
          tabindex="-1"
          style:transform={pose(index)}
          style:z-index={100 - Math.round(Math.abs(index - position.current) * 10)}
          style:opacity={clamp(1 - (Math.abs(index - position.current) - 3.2), 0, 1)}
          onclick={() => onpick(index)}
        >
          <img src={youtube.thumbnail(video.youtubeId)} alt="" draggable="false" />
        </button>
      {/each}
    </div>
    <div class="pod-flow-caption" aria-live="polite">
      <strong>{selected?.title}</strong>
      <span>{selected?.author}</span>
    </div>
  {:else}
    <p class="pod-empty">No videos yet.<br />Paste a YouTube link to add one.</p>
  {/if}
</section>
