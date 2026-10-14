<script lang="ts">
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  let { snapshot }: CaptureProps<typeof schema> = $props();
</script>
<article data-export>
  <h1>{snapshot.title}</h1>
  {#each snapshot.tasks as row (row.$id)}
    <p>{row.text}</p>
  {:else}
    <p>Empty fixture</p>
  {/each}
</article>
