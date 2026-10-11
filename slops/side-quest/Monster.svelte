<script lang="ts">
  let { color, ko, seed }: { color: string; ko: boolean; seed: number } = $props();
</script>

<svg class="sq-monster" viewBox="0 0 64 64" aria-hidden="true" data-ko={ko}>
  <path class="sq-monster-horn" d={seed % 2 ? "M18 16 L14 4 L26 13 M46 16 L50 4 L38 13" : "M20 15 Q16 4 24 8 M44 15 Q48 4 40 8"} />
  <path class="sq-monster-body" style={`fill:${color}`}
    d={seed % 3 === 0
      ? "M10 36 C8 18 22 10 33 11 C47 12 57 22 55 38 C54 50 58 57 50 56 C44 58 42 52 36 56 C30 58 26 53 20 57 C12 58 12 50 10 36 Z"
      : seed % 3 === 1
        ? "M12 30 C12 14 24 9 34 10 C48 11 54 20 54 32 C54 44 58 54 48 55 L42 50 L36 56 L30 50 L24 56 L18 50 C10 52 12 42 12 30 Z"
        : "M9 38 C6 24 16 11 32 11 C48 11 58 22 56 36 C55 48 48 57 32 57 C18 57 11 50 9 38 Z"} />
  {#if ko}
    <path class="sq-monster-eye-x" d="M20 26 l8 8 M28 26 l-8 8 M36 26 l8 8 M44 26 l-8 8" />
  {:else}
    <circle cx="24" cy="30" r="6" fill="#fff" /><circle cx="41" cy="30" r="6" fill="#fff" />
    <circle cx="25.5" cy="31" r="2.6" /><circle cx="39.5" cy="31" r="2.6" />
  {/if}
  <path class="sq-monster-mouth" d={ko ? "M24 46 q8 -5 16 0" : "M22 43 q10 8 20 0 l-3 4 l-3 -3 l-4 4 l-3 -4 l-3 3 z"} />
</svg>
