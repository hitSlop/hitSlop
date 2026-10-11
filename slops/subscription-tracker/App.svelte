<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import doc, { cadences, categories, currencies } from "./schema";
  import {
    addSubscription, blankDetails, removeSubscription, setActive, setCurrency, subscriptionView, updateSubscription,
    type Cadence, type Category, type Currency, type Details, type Scope, type Subscription,
  } from "./items";
  import { cadenceLabel, dateLabel, localDate, money, renewalState } from "./shared";
  import CategoryMark from "./CategoryMark.svelte";

  import { Button, Dialog, Select, Tabs } from "bits-ui";
  import CalendarDays from "@lucide/svelte/icons/calendar-days";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import Repeat2 from "@lucide/svelte/icons/repeat-2";

  const currencyItems = currencies.map((value) => ({ value, label: value }));
  const cadenceItems = [
    { value: "monthly", label: "Monthly" },
    { value: "annual", label: "Annual" },
  ] as const;
  const categoryItems = categories.map((value) => ({ value, label: value }));

  // What the person is looking at. Local to this window; the export shows every service.
  let view = $state<Scope>("active");
  const today = localDate(new Date());

  let showDialog = $state(false);
  let editingId = $state<string | null>(null);
  let draft = $state<Details>(blankDetails(today));

  const { rows, activeCount, pace, yearly } = $derived(subscriptionView(doc.current, view));

  function isCurrency(value: string): value is Currency {
    return currencies.some((item) => item === value);
  }
  function isCadence(value: string): value is Cadence {
    return cadences.some((item) => item === value);
  }
  function isCategory(value: string): value is Category {
    return categories.some((item) => item === value);
  }
  function beginAdd(): void {
    editingId = null;
    draft = blankDetails(localDate(new Date()));
    showDialog = true;
  }
  function beginEdit(item: Subscription): void {
    editingId = item.$id;
    draft = {
      name: item.name,
      amount: item.amount,
      cadence: item.cadence,
      nextRenewal: item.nextRenewal,
      category: item.category,
      note: item.note,
    };
    showDialog = true;
  }
  function closeDialog(): void {
    showDialog = false;
  }
  // A refused save throws before the dialog closes, so the window's notice shows why.
  async function save(): Promise<void> {
    const id = editingId;
    if (id) await doc.change((tx) => updateSubscription(tx, id, draft));
    else await doc.change((tx) => addSubscription(tx, draft));
    closeDialog();
  }
  async function toggle(item: Subscription): Promise<void> {
    await doc.change((tx) => setActive(tx, item.$id, !item.active));
  }
  async function remove(): Promise<void> {
    const id = editingId;
    if (!id) return;
    await doc.change((tx) => removeSubscription(tx, id));
    closeDialog();
  }
  async function chooseCurrency(value: string): Promise<void> {
    if (isCurrency(value)) await doc.change((tx) => setCurrency(tx, value));
  }
</script>

<main class="canvas">
  <article class="ledger" aria-label="Subscription ledger">
    <header class="masthead">
      <div>
        <h1>Subscriptions<span class="headingLoop" aria-hidden="true"><Repeat2 size={24} /></span></h1>
      </div>
      <div class="currencyField">
        <span>Currency</span>
        <Select.Root type="single" value={doc.current.currency} items={currencyItems} onValueChange={chooseCurrency}>
          <Select.Trigger class="currencyTrigger" aria-label="Document currency" title="Changes the currency label; amounts are not converted">
            <Select.Value placeholder="Currency" />
            <ChevronDown size={12} data-slop-export="hide" strokeWidth={1.8} />
          </Select.Trigger>
          <Select.Portal>
            <Select.Content class="selectContent" sideOffset={6}>
              <Select.Viewport>
                {#each currencyItems as item (item.value)}
                  <Select.Item value={item.value} label={item.label}>
                    {#snippet children({ selected })}{item.label}{#if selected}<Check size={13} strokeWidth={1.8} />{/if}{/snippet}
                  </Select.Item>
                {/each}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>
      </div>
    </header>

    <section class="totals" aria-label="Active subscription totals">
      <div>
        <span>Monthly total</span>
        <strong>{money(pace, doc.current.currency)}</strong>
      </div>
      <div>
        <span>Yearly estimate</span>
        <b>{money(yearly, doc.current.currency)}</b>
      </div>
      <div class="totalNote">
        <CalendarDays size={15} strokeWidth={1.8} />
        <span>{activeCount} active service{activeCount === 1 ? "" : "s"}</span>
      </div>
    </section>

    <section class="services" aria-labelledby="services-title">
      <div class="listHead">
        <Tabs.Root value={view} onValueChange={(value) => { if (value === "active" || value === "all") view = value; }}>
          <Tabs.List class="viewTabs" data-slop-export="hide" aria-label="Subscription views">
            <Tabs.Trigger value="active" class="viewTab">Active</Tabs.Trigger>
            <Tabs.Trigger value="all" class="viewTab">All</Tabs.Trigger>
          </Tabs.List>
        </Tabs.Root>
        <h2 id="services-title">{view === "active" ? "Active services" : "All services"}</h2>
        <Button.Root class="add" data-slop-export="hide" onclick={beginAdd}><Plus size={14} strokeWidth={1.8} /> Add service</Button.Root>
      </div>

      {#if rows.length}
        <ol class="serviceList">
          {#each rows as item (item.$id)}
            <li class="row" data-paused={!item.active}>
              <button class="serviceCopy" onclick={() => beginEdit(item)} aria-label={`Edit ${item.name}`}>
                <CategoryMark category={item.category} /><span class="serviceText"><strong>{item.name}</strong>
                <small>{item.category}{item.note ? ` · ${item.note}` : ""}</small></span>
              </button>
              <time data-state={item.active ? renewalState(item.nextRenewal, today) : "normal"}>
                <span>{!item.active ? "Paused" : renewalState(item.nextRenewal, today) === "overdue" ? "Date passed" : "Renews"}</span>
                <b>{dateLabel(item.nextRenewal)}</b>
              </time>
              <output>
                <strong>{money(item.amount, doc.current.currency)}</strong>
                <small>/{cadenceLabel(item.cadence)}</small>
              </output>
              <button class="toggle" data-slop-export="hide" onclick={() => toggle(item)} title="Changes tracking only, not your subscription" aria-label={item.active ? `Pause tracking ${item.name}` : `Resume tracking ${item.name}`}>
                {#if item.active}<Pause size={12} strokeWidth={1.8} />{:else}<Play size={12} strokeWidth={1.8} />{/if}
              </button>
            </li>
          {/each}
        </ol>
      {:else}
        <div class="empty">
          <CalendarDays size={18} strokeWidth={1.7} />
          <strong>{view === "active" && doc.current.subscriptions.length ? "Everything is paused." : "Your subscriptions, in one place."}</strong>
          <p>{view === "active" && doc.current.subscriptions.length ? "Switch to All to resume tracking a service." : "Add your first service to see its cost and next renewal."}</p>
          <Button.Root class="add" data-slop-export="hide" onclick={beginAdd}><Plus size={14} strokeWidth={1.8} /> Add your first service</Button.Root>
        </div>
      {/if}
    </section>

    <footer class="footer">
      <span>{activeCount} active service{activeCount === 1 ? "" : "s"}</span>
      <span>Tracking only · billing stays with each service</span>
    </footer>
  </article>
</main>

<Dialog.Root bind:open={showDialog}>
  <Dialog.Portal>
    <Dialog.Overlay class="editorBackdrop" data-slop-export="hide" />
    <Dialog.Content class="editor" aria-label={editingId ? "Edit subscription" : "Add subscription"} data-slop-export="hide">
      <form onsubmit={(event) => { event.preventDefault(); save(); }}>
        <header>
          <div>
            <Dialog.Title>{editingId ? "Edit subscription" : "Track a recurring cost"}</Dialog.Title>
          </div>
          <Dialog.Close class="dialogClose" type="button" aria-label="Close"><X size={14} strokeWidth={1.8} /></Dialog.Close>
        </header>
        <label>Name<input required placeholder="Service name" bind:value={draft.name} /></label>
        <div class="formGrid">
          <label>Amount<input required type="number" min="0.01" step="0.01" value={draft.amount ? draft.amount.toFixed(2) : ""} oninput={(event) => { draft.amount = Number(event.currentTarget.value); }} /></label>
          <label>Renews<input required type="date" bind:value={draft.nextRenewal} /></label>
        </div>
        <div class="formGrid">
          <label>
            Billing
            <Select.Root type="single" value={draft.cadence} items={[...cadenceItems]} onValueChange={(value) => { if (isCadence(value)) draft.cadence = value; }}>
              <Select.Trigger class="fieldTrigger" aria-label="Billing cadence">
                <Select.Value placeholder="Cadence" />
                <ChevronDown size={13} strokeWidth={1.8} />
              </Select.Trigger>
              <Select.Portal>
                <Select.Content class="selectContent" sideOffset={6}>
                  <Select.Viewport>
                    {#each cadenceItems as item (item.value)}
                      <Select.Item value={item.value} label={item.label}>
                        {#snippet children({ selected })}{item.label}{#if selected}<Check size={13} strokeWidth={1.8} />{/if}{/snippet}
                      </Select.Item>
                    {/each}
                  </Select.Viewport>
                </Select.Content>
              </Select.Portal>
            </Select.Root>
          </label>
          <label>
            Category
            <Select.Root type="single" value={draft.category} items={categoryItems} onValueChange={(value) => { if (isCategory(value)) draft.category = value; }}>
              <Select.Trigger class="fieldTrigger" aria-label="Service category">
                <Select.Value placeholder="Category" />
                <ChevronDown size={13} strokeWidth={1.8} />
              </Select.Trigger>
              <Select.Portal>
                <Select.Content class="selectContent" sideOffset={6}>
                  <Select.Viewport>
                    {#each categoryItems as item (item.value)}
                      <Select.Item value={item.value} label={item.label}>
                        {#snippet children({ selected })}{item.label}{#if selected}<Check size={13} strokeWidth={1.8} />{/if}{/snippet}
                      </Select.Item>
                    {/each}
                  </Select.Viewport>
                </Select.Content>
              </Select.Portal>
            </Select.Root>
          </label>
        </div>
        <label>Note <span class="optional">optional</span><input placeholder="Plan or detail" bind:value={draft.note} /></label>
        <footer>
          {#if editingId}<button class="deleteBtn" type="button" onclick={remove}><Trash2 size={12} strokeWidth={1.8} /> Delete</button>{/if}
          <span></span>
          <button class="cancel" type="button" onclick={closeDialog}>Cancel</button>
          <button class="save" type="submit">{editingId ? "Save changes" : "Add service"}</button>
        </footer>
      </form>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
