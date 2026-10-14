<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { mashView } from "./options";
  import Future from "./Future.svelte";

  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();
  const view = $derived(mashView(snapshot));
</script>

<article class="page export" class:preview aria-label="Exported MASH">
  <div class="coils" aria-hidden="true">{#each Array(14) as _}<i></i>{/each}</div>
  <header class="top"><h1 class="logo">M<span>·</span>A<span>·</span>S<span>·</span>H</h1></header>
  <Future {snapshot} />
  <div class="blocks">
    {#each view.blocks as block (block.slot)}
      <section class="block">
        <h3>{block.label}</h3>
        <ul>
          {#each (preview ? block.filled.slice(0, 4) : block.filled) as option (option.$id)}
            {@const order = view.fate.struck.indexOf(option.$id)}
            <li class="opt" data-struck={order >= 0 ? "" : undefined} data-pick={view.fate.picks[block.slot] === option.$id ? "" : undefined}><span>{option.text}</span></li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>
</article>

<style>
  .preview .blocks {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0 20px;
  }
</style>
