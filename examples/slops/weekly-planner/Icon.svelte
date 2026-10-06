<script lang="ts">
  import doc from "./schema";

  const allTasks = $derived(doc.current.days.flatMap((day) => day.tasks));
  const doneCount = $derived(allTasks.filter((task) => task.done).length);
</script>

<svg class="icon" viewBox="0 0 512 512" width="100%" height="100%" fill="none" aria-hidden="true">
  <rect x="26" y="32" width="460" height="456" rx="48" fill="#172e5530" />
  <rect x="26" y="24" width="460" height="456" rx="48" fill="#edf1f8" />
  <path d="M74 24h364a48 48 0 0 1 48 48v54H26V72a48 48 0 0 1 48-48" fill="#172e55" />
  {#each [96, 176, 256, 336, 416] as x}
    <rect x={x - 15} y="68" width="30" height="10" rx="5" fill="#ffffffa0" />
    <path d={`M${x + 40} 126v322`} stroke="#ccd7e7" />
  {/each}
  {#each [210, 294, 378] as y}
    <path d={`M48 ${y}h416`} stroke="#ccd7e7" />
  {/each}
  {#each [{ x: 58, y: 152, w: 64, h: 112, c: "#c8e7ff" }, { x: 138, y: 236, w: 64, h: 126, c: "#ffd2c8" }, { x: 218, y: 152, w: 64, h: 70, c: "#e0d7fc" }, { x: 298, y: 278, w: 64, h: 144, c: "#c6eedb" }, { x: 378, y: 194, w: 64, h: 84, c: "#c8e7ff" }] as block}
    <rect x={block.x} y={block.y + 5} width={block.w} height={block.h} rx="9" fill="#172e5526" />
    <rect x={block.x} y={block.y} width={block.w} height={block.h} rx="9" fill={block.c} />
    <path d={`M${block.x + 12} ${block.y + 18}h${block.w - 24}`} stroke="#172e5550" stroke-width="5" stroke-linecap="round" />
  {/each}
  {#if allTasks.length > 0 && doneCount === allTasks.length}
    <circle cx="414" cy="408" r="42" fill="#2449a5" />
    <path d="m394 409 14 14 27-30" stroke="white" stroke-width="9" stroke-linecap="round" stroke-linejoin="round" />
  {/if}
</svg>
