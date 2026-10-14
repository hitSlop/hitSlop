<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { CATEGORIES, categoryCode, formatDate } from "./shared";
  import { addItem, expenseView, removeItem, setCurrency, type Category } from "./items";
  import { bindText, getDocument } from "hitslop/svelte";
  import type { Value } from "hitslop";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { RadioGroup, Button, Tooltip } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import schema from "./schema";

  const doc = getDocument(schema);
  type ExpenseItem = Value<typeof schema.descriptor>["items"][number];

  function isCategory(value: string): value is Category {
    return CATEGORIES.some((cat) => cat.id === value);
  }

  const today = formatDate(new Date());
  let draftTitle = $state("");
  let draftAmount = $state("");
  let draftCategory = $state<Category>("food");
  let titleRef = $state<HTMLInputElement>();

  const view = $derived(expenseView(doc.current));
  const canAdd = $derived(Boolean(draftTitle.trim()) && Number(draftAmount) > 0);
  const flipMs = $derived(prefersReducedMotion.current ? 0 : 220);

  function submitItem(): void {
    doc.change(tx => addItem(tx, { title: draftTitle, amount: parseFloat(draftAmount), category: draftCategory }, new Date()));
    draftTitle = "";
    draftAmount = "";
    titleRef?.focus();
  }
</script>

{#snippet receiptRows(items: readonly ExpenseItem[], editable: boolean)}
  <ul class="list">
    {#each items as item (item.$id)}
      <li class="row" animate:flip={{ duration: editable ? flipMs : 0 }}>
        <div class="itemInfo">
          {#if editable}
            <input class="itemTitle" use:bindText={doc.at(item).title} aria-label="Item description" />
          {:else}
            <span class="itemTitle">{item.title.trim() || "Untitled item"}</span>
          {/if}
          <span class="itemTag">{categoryCode(item.category)} · {item.time}</span>
        </div>
        {#if editable}
          <div class="itemRight">
            <strong class="itemCost">{view.money(item.amount)}</strong>
            <Tooltip.Root>
              <Tooltip.Trigger class="remove" data-slop-export="hide" aria-label={`Remove ${item.title || "untitled item"}`} onclick={() => doc.change(tx => removeItem(tx, item.$id))}>
                <Trash2 size={13} />
              </Tooltip.Trigger>
              <Tooltip.Portal>
                <Tooltip.Content class="tooltip" sideOffset={6}>VOID LINE</Tooltip.Content>
              </Tooltip.Portal>
            </Tooltip.Root>
          </div>
        {:else}
          <strong class="itemCost">{view.money(item.amount)}</strong>
        {/if}
      </li>
    {:else}
      <li class="empty">
        <strong>NO TRANSACTIONS</strong>
        <p>{editable ? "Add a purchase above to start the roll." : "Nothing printed on this roll."}</p>
      </li>
    {/each}
  </ul>
{/snippet}

{#snippet tear(edge: "top" | "bottom")}
  <div class="tear" data-edge={edge === "bottom" ? "bottom" : undefined} aria-hidden="true">
    {#each Array.from({ length: 12 }) as _, index (index)}<i></i>{/each}
  </div>
{/snippet}

  <Tooltip.Provider>
    <main class="shell" aria-label="Expense receipt">
      {@render tear("top")}

      <article class="paper">
        <header class="masthead">
          <p class="stamp">*** RECEIPT ***</p>
          <input class="storeTitle" use:bindText={doc.fields.storeName} aria-label="Store title" />
          <div class="meta">
            <label class="terminal">
              <span>TERM</span>
              <input use:bindText={doc.fields.terminalId} aria-label="Terminal id" />
            </label>
            <span>DATE {today}</span>
          </div>
        </header>

        <form class="composer" data-slop-export="hide" onsubmit={(event) => { event.preventDefault(); submitItem(); }}>
          <div class="composerInputs">
            <input
              bind:this={titleRef}
              bind:value={draftTitle}
              class="titleInput"
              placeholder="Item"
              aria-label="Item title"
              required
            />
            <div class="amountWrap">
              <input
                class="currency"
                value={doc.current.currency}
                oninput={(event) => doc.change(tx => setCurrency(tx, event.currentTarget.value))}
                aria-label="Currency symbol"
                maxlength="3"
              />
              <input
                bind:value={draftAmount}
                class="amountInput"
                type="number"
                step="0.01"
                min="0.01"
                placeholder="0.00"
                aria-label="Amount"
                required
              />
            </div>
          </div>
          <div class="composerBottom">
            <RadioGroup.Root
              class="catGroup"
              orientation="horizontal"
              value={draftCategory}
              onValueChange={(value) => { if (isCategory(value)) draftCategory = value; }}
              aria-label="Purchase category"
            >
              {#each CATEGORIES as cat (cat.id)}
                <RadioGroup.Item value={cat.id} class="catPill" aria-label={cat.label}>
                  {#snippet children()}{cat.code}{/snippet}
                </RadioGroup.Item>
              {/each}
            </RadioGroup.Root>
            <Button.Root class="add" type="submit" aria-label="Add purchase" disabled={!canAdd}><Plus size={16} /></Button.Root>
          </div>
        </form>

        <section class="items" aria-label="Recorded expenses">
          <div class="tableHead">
            <span>ITEM</span>
            <span>AMT</span>
          </div>
          {@render receiptRows(view.items, true)}
        </section>

        <footer class="footer">
          {#if view.categoryTotals.length > 0}
            <div class="breakdown">
              <p class="breakdownTitle">CATEGORY TOTALS</p>
              {#each view.categoryTotals as [cat, catSum] (cat)}
                <div class="breakdownRow">
                  <span>{categoryCode(cat)}</span>
                  <span>{view.money(catSum)}</span>
                </div>
              {/each}
            </div>
          {/if}

          <div class="grand" aria-live="polite">
            <span>TOTAL · {view.count}</span>
            <strong class="totalDigits">{view.money(view.total)}</strong>
          </div>

          <div class="barcodeBlock" aria-hidden="true">
            <div class="barcode">
              <i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i>
              <i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i><i></i>
              <i></i><i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i>
            </div>
            <span class="barcodeLabel">{today.replaceAll("-", "")}-{view.totalCode}</span>
            <span class="barcodeLabel">THANK YOU FOR LOGGING</span>
          </div>
        </footer>
      </article>

      {@render tear("bottom")}
    </main>
  </Tooltip.Provider>
