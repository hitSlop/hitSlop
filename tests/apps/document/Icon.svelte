<script lang="ts">
  import doc from "./schema";
  const count = $derived(doc.current.tasks.filter(row => row.done).length);
</script>
<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512" aria-label="Fixture icon">
  <rect x="32" y="32" width="448" height="448" rx="48" fill="var(--slop-accent)" />
  <rect x="64" y="64" width={32 + count * 24} height="128" fill="white" />
</svg>
