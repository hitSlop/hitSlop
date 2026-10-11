<script lang="ts">
  // A still fogged mirror: the room behind it (or the last saved snapshot), frost over the
  // glass and a finger-drawn smile. Used by the start
  // screen, the export and the icon; it never touches a device.
  let { image = null, alt = "", variant = "start" }: {
    image?: string | null;
    alt?: string;
    variant?: "start" | "export" | "icon";
  } = $props();
  const id = $props.id();
</script>

<svg class="fg-glass" data-variant={variant} viewBox="0 0 400 300" preserveAspectRatio="xMidYMid slice" role={image ? "img" : undefined} aria-label={image ? alt : undefined} aria-hidden={image ? undefined : "true"}>
  <defs>
    <linearGradient id="{id}-room" x1="0" y1="0" x2="0.4" y2="1">
      <stop offset="0" class="fg-room-top" />
      <stop offset="1" class="fg-room-bottom" />
    </linearGradient>
    <radialGradient id="{id}-lamp" cx="0.72" cy="0.18" r="0.55">
      <stop offset="0" class="fg-lamp" stop-opacity="0.5" />
      <stop offset="1" class="fg-lamp" stop-opacity="0" />
    </radialGradient>
    <filter id="{id}-soft" filterUnits="userSpaceOnUse" x="-20" y="-20" width="440" height="340">
      <feGaussianBlur stdDeviation={variant === "icon" ? 2 : 3.2} />
    </filter>
    <filter id="{id}-grain" x="0" y="0" width="100%" height="100%">
      <feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="2" seed="7" />
      <feColorMatrix values="0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 0.55 0" />
    </filter>
    <mask id="{id}-wiped" maskUnits="userSpaceOnUse" x="0" y="0" width="400" height="300">
      <rect width="400" height="300" fill="white" />
      <g filter="url(#{id}-soft)" stroke="black" fill="none" stroke-linecap="round" stroke-linejoin="round">
        {#if variant === "icon"}
          <circle cx="200" cy="146" r="90" stroke-width="24" />
          <path d="M168 118 v 6 M232 118 v 6" stroke-width="30" />
          <path d="M152 166 Q 200 212 248 166" stroke-width="22" />
        {:else}
          <path d="M-26 88 Q 22 8 97 28" stroke-width="26" />
          <path d="M340 257v2m26-2v2M339 271q14 14 28 0" stroke-width="5" />
        {/if}
      </g>
    </mask>
  </defs>
  {#if image}
    <image href={image} width="400" height="300" preserveAspectRatio="xMidYMid slice" />
  {:else}
    <rect width="400" height="300" fill="url(#{id}-room)" />
    <rect width="400" height="300" fill="url(#{id}-lamp)" />
    <rect class="fg-tile-line" x="0" y="212" width="400" height="1" />
    <rect class="fg-tile-line" x="132" y="0" width="1" height="300" />
    <rect class="fg-tile-line" x="266" y="0" width="1" height="300" />
  {/if}
  {#if !image || variant !== "export"}
    <g mask="url(#{id}-wiped)">
      <rect class="fg-frost" width="400" height="300" />
      <rect width="400" height="300" filter="url(#{id}-grain)" opacity="0.07" />
    </g>
  {/if}
  {#if !image && variant === "export"}
    <text class="fg-glass-title" x="200" y="160" text-anchor="middle">fog</text>
  {/if}
</svg>
