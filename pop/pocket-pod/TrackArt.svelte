<script lang="ts">
import { thumbnail } from "@hitslop/pop/youtube";
import type { Video } from "./pod";
let { video, class: className = "" }: { video: Video; class?: string } = $props();
</script>

{#if video.audio}
  <svg class={className} viewBox="0 0 160 160" aria-hidden="true">
    <rect width="160" height="160" fill="var(--pop-accent)" />
    <circle cx="80" cy="80" r="64" fill="#202126" />
    <circle cx="80" cy="80" r="51" fill="none" stroke="#ffffff" stroke-opacity=".12" stroke-width="12" />
    <circle cx="80" cy="80" r="29" fill="var(--pop-button)" />
    <path d="M75 66v29m0-23 18-4v22" fill="none" stroke="#202126" stroke-width="5" />
    <ellipse cx="69" cy="95" rx="8" ry="6" fill="#202126" /><ellipse cx="87" cy="90" rx="8" ry="6" fill="#202126" />
  </svg>
{:else}
  <img class={className} src={thumbnail(video.youtubeId)} alt="" draggable="false" />
{/if}
