<script lang="ts">
  import doc from "./schema";
</script>
<article data-export>
  <h1>{doc.current.title}</h1>
  {#each doc.current.tasks as row (row.$id)}
    <p>{row.text}</p>
  {:else}
    <p>Empty fixture</p>
  {/each}
</article>
