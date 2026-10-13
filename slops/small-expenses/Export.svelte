<script lang="ts">
  import { previewText } from "../_shared/preview";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { expenseView, formatMoney } from "./items";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();
  const view = $derived(expenseView(snapshot));
  const money = (minor: number) => formatMoney(minor, snapshot.currency);
</script>

<main class="expenses-paper expenses-export">
  <div class="expenses-eyebrow">Small expenses · {snapshot.currency}</div>
  <h1 class="expenses-title">{snapshot.title || "Small expenses"}</h1>
  <div class="expenses-overview"><div><div class="expenses-muted">TOTAL RECORDED</div><div class="expenses-total">{money(view.total)}</div><div class="expenses-muted">{money(view.unsettled)} still to settle</div></div></div>
  <ul class="expenses-list">
    {#each (preview ? (snapshot.items).slice(0, 7) : snapshot.items) as row}
      <li class="expenses-row"><span>{row.settled ? "✓" : "○"}</span><div><strong class="expenses-merchant">{row.merchant}</strong>{#if row.note}<p class="expenses-note">{previewText(row.note, preview, 90)}</p>{/if}</div><div class="expenses-amount">{money(row.amountMinor)}<div class="expenses-muted">{row.settled ? "Settled" : "Unsettled"}</div></div></li>
    {:else}<li class="expenses-note">Nothing recorded yet.</li>{/each}
  </ul>
  <footer class="expenses-footer">{view.count} expenses · Amounts as entered</footer>
</main>
<style>.expenses-export { height: auto; overflow: visible; } .expenses-export :global(.expenses-title) { white-space: pre-wrap; overflow-wrap: anywhere; }</style>
