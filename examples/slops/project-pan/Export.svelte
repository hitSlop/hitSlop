<script lang="ts">
  import doc from "./schema";
  import { inUse, panned } from "./pan";
  import Pan from "./Pan.svelte";

  const active = $derived(inUse(doc.current.products));
  const empties = $derived(panned(doc.current.products));
</script>

<article class="desk export" aria-label="Exported project pan">
  <section class="compact">
    <header class="mirror">
      <h1 class="title">{doc.current.title || "Project Pan"}</h1>
      <p class="count"><b>{empties.length}</b> panned · <b>{active.length}</b> in use</p>
    </header>
    <div class="tray">{#each active as product (product.$id)}<Pan {product} />{/each}</div>
    <section class="shelf">
      <h2>Empties <span>{empties.length}</span></h2>
      <div class="shelf-row">{#each empties as product (product.$id)}<Pan {product} />{:else}<p class="none">Nothing panned yet.</p>{/each}</div>
    </section>
  </section>
</article>
