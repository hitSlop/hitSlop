<script lang="ts">
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { RadioGroup, Button, Tooltip } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import schema, { categories, type ExpenseItem } from "./schema";

  const CATEGORIES = [
    { id: "food", label: "Food", code: "FOOD" },
    { id: "transit", label: "Transit", code: "TRANSIT" },
    { id: "coffee", label: "Coffee", code: "COFFEE" },
    { id: "gear", label: "Gear", code: "GEAR" },
    { id: "bills", label: "Bills", code: "BILLS" },
    { id: "other", label: "Other", code: "MISC" },
  ] as const;
  type ExpenseCategory = (typeof categories)[number];

  function formatTime(d: Date): string {
    return d.toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", hour12: false });
  }
  function formatDate(d: Date): string {
    const year = d.getFullYear();
    const month = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${year}-${month}-${day}`;
  }
  function isCategory(value: string): value is ExpenseCategory {
    return CATEGORIES.some((cat) => cat.id === value);
  }
  function categoryCode(id: string): string {
    return CATEGORIES.find((cat) => cat.id === id)?.code ?? "MISC";
  }

  const today = formatDate(new Date());
  const doc = useDocument(schema);

  let draftTitle = $state("");
  let draftAmount = $state("");
  let draftCategory = $state<ExpenseCategory>("food");
  let titleRef = $state<HTMLInputElement>();

  const total = $derived(doc.current.items.reduce((sum, item) => sum + Number(item.amount || 0), 0));
  const categoryTotals = $derived.by(() => {
    const map = new Map<string, number>();
    for (const item of doc.current.items) {
      map.set(item.category, (map.get(item.category) ?? 0) + Number(item.amount || 0));
    }
    return Array.from(map.entries()).sort((a, b) => b[1] - a[1]);
  });
  const canAdd = $derived(Boolean(draftTitle.trim()) && Number(draftAmount) > 0);
  const flipMs = $derived(prefersReducedMotion.current ? 0 : 220);
  const symbol = $derived(doc.current.currency.trim() || "$");

  function addItem(): void {
    const title = draftTitle.trim();
    const amountNum = parseFloat(draftAmount);
    if (!title || Number.isNaN(amountNum) || amountNum <= 0) return;
    const currentTime = new Date();
    doc.fields.items.insert({
      title,
      amount: Math.round(amountNum * 100) / 100,
      category: draftCategory,
      date: formatDate(currentTime),
      time: formatTime(currentTime),
    });
    draftTitle = "";
    draftAmount = "";
    titleRef?.focus();
  }

  function removeItem(id: string): void {
    doc.fields.items.remove(id);
  }

  function formatMoney(amount: number): string {
    return `${symbol}${amount.toFixed(2)}`;
  }

  function clampCurrency(value: string): string {
    const next = value.replace(/\s/g, "").slice(0, 3);
    return next.length > 0 ? next : "$";
  }

  function label(title: string): string {
    return title.trim().toUpperCase().slice(0, 12) || "ITEM";
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
            <strong class="itemCost">{formatMoney(item.amount)}</strong>
            <Tooltip.Root>
              <Tooltip.Trigger class="remove" data-slop-export="hide" aria-label={`Remove ${item.title || "untitled item"}`} onclick={() => removeItem(item.$id)}>
                <Trash2 size={13} />
              </Tooltip.Trigger>
              <Tooltip.Portal>
                <Tooltip.Content class="tooltip" sideOffset={6}>VOID LINE</Tooltip.Content>
              </Tooltip.Portal>
            </Tooltip.Root>
          </div>
        {:else}
          <strong class="itemCost">{formatMoney(item.amount)}</strong>
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

<Slop>
  <Tooltip.Provider>
    <main class="shell" data-slop-selection="none" aria-label="Expense receipt">
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

        <form class="composer" data-slop-export="hide" onsubmit={(event) => { event.preventDefault(); addItem(); }}>
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
                oninput={(event) => doc.fields.currency.set(clampCurrency(event.currentTarget.value))}
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
          {@render receiptRows(doc.current.items, true)}
        </section>

        <footer class="footer">
          {#if categoryTotals.length > 0}
            <div class="breakdown">
              <p class="breakdownTitle">CATEGORY TOTALS</p>
              {#each categoryTotals as [cat, catSum] (cat)}
                <div class="breakdownRow">
                  <span>{categoryCode(cat)}</span>
                  <span>{formatMoney(catSum)}</span>
                </div>
              {/each}
            </div>
          {/if}

          <div class="grand" aria-live="polite">
            <span>TOTAL · {doc.current.items.length}</span>
            <strong class="totalDigits">{formatMoney(total)}</strong>
          </div>

          <div class="barcodeBlock" aria-hidden="true">
            <div class="barcode">
              <i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i>
              <i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i><i></i>
              <i></i><i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i>
            </div>
            <span class="barcodeLabel">{today.replaceAll("-", "")}-{String(Math.round(total * 100)).padStart(6, "0")}</span>
            <span class="barcodeLabel">THANK YOU FOR LOGGING</span>
          </div>
        </footer>
      </article>

      {@render tear("bottom")}
    </main>
  </Tooltip.Provider>

  {#snippet exportView()}
    <article class="exportShell" aria-label="Exported expense receipt">
      {@render tear("top")}
      <div class="paper">
        <header class="masthead">
          <p class="stamp">*** RECEIPT ***</p>
          <h1 class="storeTitle">{doc.current.storeName.trim() || "EXPENSE LOG"}</h1>
          <div class="meta">
            <span>TERM {doc.current.terminalId.trim() || "—"}</span>
            <span>DATE {today}</span>
          </div>
        </header>
        <section class="items" aria-label="Recorded expenses">
          <div class="tableHead">
            <span>ITEM</span>
            <span>AMT</span>
          </div>
          {@render receiptRows(doc.current.items, false)}
        </section>
        <footer class="footer">
          {#if categoryTotals.length > 0}
            <div class="breakdown">
              <p class="breakdownTitle">CATEGORY TOTALS</p>
              {#each categoryTotals as [cat, catSum] (cat)}
                <div class="breakdownRow">
                  <span>{categoryCode(cat)}</span>
                  <span>{formatMoney(catSum)}</span>
                </div>
              {/each}
            </div>
          {/if}
          <div class="grand">
            <span>TOTAL · {doc.current.items.length}</span>
            <strong class="totalDigits">{formatMoney(total)}</strong>
          </div>
          <div class="barcodeBlock" aria-hidden="true">
            <div class="barcode">
              <i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i>
              <i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i><i></i>
              <i></i><i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i>
            </div>
            <span class="barcodeLabel">{today.replaceAll("-", "")}-{String(Math.round(total * 100)).padStart(6, "0")}</span>
            <span class="barcodeLabel">THANK YOU FOR LOGGING</span>
          </div>
        </footer>
      </div>
      {@render tear("bottom")}
    </article>
  {/snippet}

  {#snippet icon()}
    {@const lines = doc.current.items.slice(0, 4)}
    {@const placeholders = [
      { title: "COFFEE", amount: 4.5 },
      { title: "METRO", amount: 2.9 },
      { title: "LUNCH", amount: 14.8 },
      { title: "BOOKS", amount: 22 },
    ]}
    {@const shown = lines.length ? lines : placeholders}
    {@const shownTotal = lines.length ? total : placeholders.reduce((sum, item) => sum + item.amount, 0)}
    <div class="iconSurface" aria-hidden="true">
      <article class="iconReceipt">
        <div class="iconTear">
          {#each Array.from({ length: 8 }) as _, index (index)}<i></i>{/each}
        </div>
        <div class="iconBody">
          <div class="iconHeader">
            <span class="iconTitle">EXPENSE LOG</span>
            <span class="iconSub">{doc.current.terminalId.trim() || "TERMINAL"}</span>
          </div>
          <div class="iconDash"></div>
          <div class="iconItems">
            {#each shown as item, index (index)}
              <div class="iconRow"><span>{label(item.title)}</span><strong>{formatMoney(item.amount)}</strong></div>
            {/each}
          </div>
          <div class="iconDouble"></div>
          <div class="iconTotal">
            <span>TOTAL</span>
            <strong>{formatMoney(shownTotal)}</strong>
          </div>
          <div class="iconBarcode">
            <i></i><i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i>
            <i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i><i></i>
          </div>
        </div>
      </article>
    </div>
  {/snippet}
</Slop>
