<script lang="ts">
  import doc, { slots } from "./schema";
  import { play, slotLabel } from "./fate";
  import Future from "./Future.svelte";

  const fate = $derived(play(doc.current.options, doc.current.loops));
</script>

<article class="page export" aria-label="Exported MASH">
  <div class="coils" aria-hidden="true">{#each Array(14) as _}<i></i>{/each}</div>
  <header class="top"><h1 class="logo">M<span>·</span>A<span>·</span>S<span>·</span>H</h1></header>
  <Future />
  <div class="blocks">
    {#each slots as slot (slot)}
      <section class="block">
        <h3>{slotLabel[slot]}</h3>
        <ul>
          {#each doc.current.options.filter((option) => option.slot === slot && option.text.trim()) as option (option.$id)}
            {@const order = fate.struck.indexOf(option.$id)}
            <li class="opt" data-struck={order >= 0 ? "" : undefined} data-pick={fate.picks[slot] === option.$id ? "" : undefined}><span>{option.text}</span></li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>
</article>
