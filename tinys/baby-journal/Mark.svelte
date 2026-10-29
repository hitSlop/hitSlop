<script lang="ts">
  let { kind, size = 32 }: { kind: string; size?: number } = $props();
</script>
<svg width={size} height={size} viewBox="0 0 48 48" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  {#if kind === "feeding"}
    <g transform="rotate(17 24 24)"><path d="M20 11V7a4 4 0 0 1 8 0v4" fill="#fffdf8"/><path d="M17 12h14v7H17z" fill="#d2e3f1"/><rect x="14" y="19" width="20" height="25" rx="6" fill="#fffdf8"/><path d="M26 26h7m-5 6h5m-7 6h7" stroke="#eda989"/></g>
  {:else if kind === "diaper"}
    <path d="M5 13h38l-3 20c-1 7-8 10-16 10S9 40 8 33Z" fill="#fffdf8"/><path d="M6 21h9l-1 7H8m34-7h-9l1 7h7M11 32c7-1 10 4 10 10m16-10c-7-1-10 4-10 10"/><path d="M18 20h12" stroke="#9ec9b8"/>
  {:else if kind === "sleep"}
    <path d="M32 5C16 2 4 15 7 29c3 15 25 20 35 4C24 39 17 18 32 5Z" fill="#fffdf8"/><path d="m37 8 1 4 4 1-4 1-1 4-1-4-4-1 4-1Z" fill="#ffe294" stroke-width="1.5"/>
  {:else if kind === "note"}
    <path d="M13 9h24v32H11V12a3 3 0 0 1 3-3Z" fill="#fffdf8"/><path d="M11 9H8v32h5m6-12h11m-11 6h8"/><path d="M24 23s-8-5-5-8c2-2 5 0 5 1 1-2 4-3 6-1 3 3-6 8-6 8Z" fill="#ffc1a6" stroke-width="1.7"/>
  {:else if kind === "sun"}
    <circle cx="24" cy="24" r="12" fill="#ffdc7b" stroke="none"/><path d="M19 26q5 7 10 0" stroke-width="2"/><path d="M19 21v1m10-1v1"/><path d="M24 3v5m0 32v5M3 24h5m32 0h5M9 9l4 4m22 22 4 4M9 39l4-4m22-22 4-4" stroke="#ecc363"/>
  {:else if kind === "heart"}
    <path d="M24 39S4 26 7 15c3-10 14-7 17 1 3-8 14-11 17-1 3 11-17 24-17 24Z"/>
  {:else if kind === "plus"}<path d="M24 10v28M10 24h28"/>
  {:else if kind === "edit"}<path d="m10 33-2 8 8-2L39 16l-6-6Zm18-18 6 6M8 41l8-2"/>
  {:else if kind === "chevron"}<path d="m19 13 11 11-11 11"/>
  {:else if kind === "close"}<path d="m13 13 22 22m0-22L13 35"/>
  {:else if kind === "camera"}<path d="M5 15h10l3-5h12l3 5h10v26H5Z"/><circle cx="24" cy="27" r="8"/>
  {:else if kind === "people"}<circle cx="18" cy="16" r="7"/><path d="M4 41v-5c0-15 28-15 28 0v5m0-31c10 0 10 13 0 13m6 6c5 2 6 6 6 12"/>
  {:else}<path d="M10 24h.1M24 24h.1M38 24h.1" stroke-width="5"/>{/if}
</svg>
