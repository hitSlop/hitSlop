<script lang="ts">
  let { children }: { children: import("svelte").Snippet } = $props();
</script>

<p class="preview-note">{@render children()}</p>

<style>
  .preview-note { margin: 0; padding: 10px 16px; color: #625f58; background: #f7f5ef; font-family: sans-serif; font-size: 12px; font-weight: 400; line-height: 1.4; }
</style>
