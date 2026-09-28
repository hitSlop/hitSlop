<script lang="ts">
  import { Checkbox, Tabs } from "bits-ui";
  import { Slop, useDocument, bindText } from "@hitslop/document/svelte";
  import { tick } from "svelte";
  import { expenses } from "./schema";

  const doc=useDocument(expenses);
  const { title, currency, items } = doc.fields;
  const merchantInputs = new Map<string, HTMLInputElement>();
  function merchantInput(node: HTMLInputElement, id: string) {
    merchantInputs.set(id, node);
    return { destroy() { merchantInputs.delete(id); } };
  }
  let merchant=$state(""), amount=$state(""), note=$state(""), error=$state("");
  let selected=$state<string[]>([]);
  const selectedIDs=$derived(selected.filter(id=>doc.current.items.some(row=>row.$id===id)));
  const total=$derived(doc.current.items.reduce((sum,row)=>sum+row.amountMinor,0));
  const unsettled=$derived(doc.current.items.filter(row=>!row.settled).reduce((sum,row)=>sum+row.amountMinor,0));
  const money=(minor:number)=>new Intl.NumberFormat("en-CA",{style:"currency",currency:doc.current.currency}).format(minor/100);
  async function add() {
    if(!merchant.trim()){error="Add a merchant.";return;}
    if(!/^\d+(\.\d{1,2})?$/.test(amount.trim())){error="Enter an amount with at most two decimal places.";return;}
    const [whole,fraction=""]=amount.trim().split(".");
    const minor=Number(whole)*100+Number(fraction.padEnd(2,"0"));
    if(!Number.isSafeInteger(minor)){error="Amount is too large.";return;}
    error="";
    const { id } = items.insert({merchant:merchant.trim(),amountMinor:minor,...(note.trim()?{note:note.trim()}:{}),settled:false});
    merchant="";amount="";note="";
    await tick();
    merchantInputs.get(id)?.focus();
  }
  function settle() { doc.change(tx=>{for(const id of selectedIDs)tx.fields.items.item(id).settled.set(true)},{message:"Settle expenses"});selected=[]; }
</script>
<Slop>
<main class="expenses-paper">
  <div class="expenses-eyebrow">Small expenses</div>
  <input class="expenses-title" aria-label="List title" use:bindText={title} />
  <div class="expenses-overview">
    <div><div class="expenses-muted">TOTAL RECORDED</div><div class="expenses-total" data-total>{money(total)}</div><div class="expenses-muted">{money(unsettled)} still to settle</div></div>
    <Tabs.Root value={doc.current.currency} onValueChange={value=>currency.set(value as "CAD"|"USD"|"EUR")}>
      <Tabs.List class="expenses-currencies" aria-label="Currency">
        {#each ["CAD","USD","EUR"] as currency}<Tabs.Trigger class="expenses-currency" value={currency}>{currency}</Tabs.Trigger>{/each}
      </Tabs.List>
    </Tabs.Root>
  </div>
  <div class="expenses-toolbar">
    <div class="expenses-selection"><Checkbox.Root class="expenses-check" aria-label="Select all expenses" checked={doc.current.items.length>0 && selectedIDs.length===doc.current.items.length} onCheckedChange={checked=>selected=checked?doc.current.items.map(row=>row.$id):[]}>
      {#if doc.current.items.length>0 && selectedIDs.length===doc.current.items.length}✓{/if}
    </Checkbox.Root><span class="expenses-muted">{selectedIDs.length?`${selectedIDs.length} selected`:`${doc.current.items.length} expenses`}</span></div>
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
            <button class="expenses-small" disabled={index===0} aria-label="Move expense up" onclick={()=>items.move(row.$id,{before:doc.current.items[index-1]!.$id})}>↑</button>
            <button class="expenses-small" disabled={index===doc.current.items.length-1} aria-label="Move expense down" onclick={()=>items.move(row.$id,{after:doc.current.items[index+1]!.$id})}>↓</button>
            {#if row.note!==undefined}<button class="expenses-small" onclick={()=>doc.at(row).note.clear()}>Clear note</button>{/if}
            <button class="expenses-small" onclick={()=>items.remove(row.$id)}>Remove</button>
          </div>
        </div>
        <div class="expenses-amount">{money(row.amountMinor)}<div class="expenses-muted">{row.settled?"Settled":"Unsettled"}</div></div>
      </li>
    {/each}
  </ul>
  {#if !doc.current.items.length}<p class="expenses-note">Nothing recorded yet. Add your first expense below.</p>{/if}
  <form class="expenses-form" onsubmit={event=>{event.preventDefault();return add()}}>
    <input class="expenses-entry" aria-label="Merchant" placeholder="Merchant" bind:value={merchant} />
    <input class="expenses-entry" aria-label="Amount" inputmode="decimal" placeholder="0.00" bind:value={amount} />
    <input class="expenses-entry" aria-label="Note" placeholder="Note (optional)" bind:value={note} />
    <button class="expenses-button" disabled={!merchant.trim()||!amount.trim()}>Add</button>
  </form>
  {#if error}<p class="expenses-error" role="alert">{error}</p>{/if}
  <footer class="expenses-footer"><span>Amounts stay as entered; currency changes the label.</span></footer>
</main>
</Slop>
