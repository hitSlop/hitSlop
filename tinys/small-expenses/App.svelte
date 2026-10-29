<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import { Checkbox, Tabs } from "bits-ui";
  import { bindText, getDocument } from "@hitslop/tinys/svelte";
  import { tick } from "svelte";
  import schema from "./schema";
  import { addItem, clearNote, expenseView, formatMoney, moveItem, parseEntry, removeItem, settleItems } from "./items";

  const doc = getDocument(schema);
  const { title, currency } = doc.fields;
  const merchantInputs = new Map<string, HTMLInputElement>();
  function merchantInput(node: HTMLInputElement, id: string) {
    merchantInputs.set(id, node);
    return { destroy() { merchantInputs.delete(id); } };
  }
  let merchant=$state(""), amount=$state(""), note=$state(""), error=$state("");
  let selected=$state<string[]>([]);
  const selectedIDs=$derived(selected.filter(id=>doc.current.items.some(row=>row.$id===id)));
  const view=$derived(expenseView(doc.current));
  const money=(minor:number)=>formatMoney(minor, doc.current.currency);
  async function add() {
    const entry=parseEntry(merchant, amount, note);
    if("error" in entry){error=entry.error;return;}
    error="";
    // The id arrives with acceptance; a refused entry stays filled for retry.
    const { id } = await doc.change(tx=>addItem(tx, entry));
    merchant="";amount="";note="";
    await tick();
    merchantInputs.get(id)?.focus();
  }
  async function settle() {
    const settling=selectedIDs;
    // A refused change keeps the selection.
    await doc.change(tx=>settleItems(tx, settling));
    selected=[];
  }
</script>

<main class="expenses-paper">
  <div class="expenses-eyebrow">Small expenses</div>
  <input class="expenses-title" aria-label="List title" use:bindText={title} />
  <div class="expenses-overview">
    <div><div class="expenses-muted">TOTAL RECORDED</div><div class="expenses-total" data-total>{money(view.total)}</div><div class="expenses-muted">{money(view.unsettled)} still to settle</div></div>
    <Tabs.Root bind:value={currency.value}>
      <Tabs.List class="expenses-currencies" aria-label="Currency">
        {#each ["CAD","USD","EUR"] as currency}<Tabs.Trigger class="expenses-currency" value={currency}>{currency}</Tabs.Trigger>{/each}
      </Tabs.List>
    </Tabs.Root>
  </div>
  <div class="expenses-toolbar">
    <div class="expenses-selection"><Checkbox.Root class="expenses-check" aria-label="Select all expenses" checked={view.count>0 && selectedIDs.length===view.count} onCheckedChange={checked=>selected=checked?doc.current.items.map(row=>row.$id):[]}>
      {#if view.count>0 && selectedIDs.length===view.count}✓{/if}
    </Checkbox.Root><span class="expenses-muted">{selectedIDs.length?`${selectedIDs.length} selected`:`${view.count} expenses`}</span></div>
    <button class="expenses-button" aria-label="Settle selected expenses" disabled={!selectedIDs.length} onclick={settle}>Settle selected</button>
  </div>
  <ul class="expenses-list">
    {#each doc.current.items as row,index (row.$id)}
      <li class="expenses-row" data-row-id={row.$id}>
        <Checkbox.Root class="expenses-check" aria-label={`Select ${row.merchant}`} checked={selectedIDs.includes(row.$id)} onCheckedChange={checked=>selected=checked?[...selectedIDs,row.$id]:selectedIDs.filter(id=>id!==row.$id)}>{#if selectedIDs.includes(row.$id)}✓{/if}</Checkbox.Root>
        <div>
          <input class="expenses-merchant" aria-label="Merchant text" use:bindText={doc.at(row).merchant} use:merchantInput={row.$id} />
          {#if row.note}<p class="expenses-note">{row.note}</p>{/if}
          <div class="expenses-actions">
            <button class="expenses-small" disabled={index===0} aria-label="Move expense up" onclick={()=>doc.change(tx=>moveItem(tx,row.$id,{before:doc.current.items[index-1]!.$id}))}>↑</button>
            <button class="expenses-small" disabled={index===view.count-1} aria-label="Move expense down" onclick={()=>doc.change(tx=>moveItem(tx,row.$id,{after:doc.current.items[index+1]!.$id}))}>↓</button>
            {#if row.note!==undefined}<button class="expenses-small" onclick={()=>doc.change(tx=>clearNote(tx,row.$id))}>Clear note</button>{/if}
            <button class="expenses-small" onclick={()=>doc.change(tx=>removeItem(tx,row.$id))}>Remove</button>
          </div>
        </div>
        <div class="expenses-amount">{money(row.amountMinor)}<div class="expenses-muted">{row.settled?"Settled":"Unsettled"}</div></div>
      </li>
    {/each}
  </ul>
  {#if !view.count}<p class="expenses-note">Nothing recorded yet. Add your first expense below.</p>{/if}
  <form class="expenses-form" onsubmit={event=>{event.preventDefault();return add()}}>
    <input class="expenses-entry" aria-label="Merchant" placeholder="Merchant" bind:value={merchant} />
    <input class="expenses-entry" aria-label="Amount" inputmode="decimal" placeholder="0.00" bind:value={amount} />
    <input class="expenses-entry" aria-label="Note" placeholder="Note (optional)" bind:value={note} />
    <button class="expenses-button" disabled={!merchant.trim()||!amount.trim()}>Add</button>
  </form>
  {#if error}<p class="expenses-error" role="alert">{error}</p>{/if}
  <footer class="expenses-footer"><span>Amounts stay as entered; currency changes the label.</span></footer>
</main>
