<script lang="ts">
  // A kept photo as a taped-in print. Used by the editor list, the large view and the export.
  let { src, alt, width, height, size = "thumb" }: {
    src: string;
    alt: string;
    width?: number;
    height?: number;
    size?: "thumb" | "large" | "export";
  } = $props();
  let failed = $state(false);
</script>

<span class="cn-print" data-size={size} style:--cn-ratio={width && height ? `${width} / ${height}` : "4 / 3"}>
  <span class="cn-tape-strip" aria-hidden="true"></span>
  {#if failed}
    <span class="cn-print-missing">Photo unavailable</span>
  {:else}
    <img {src} {alt} {width} {height} loading={size === "thumb" ? "lazy" : "eager"} decoding="async" onerror={() => (failed = true)} />
  {/if}
</span>
