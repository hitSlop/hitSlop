<script lang="ts">
  import { Slop, useDocument } from "@hitslop/document/svelte";
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
  import schema, { cadences, categories, currencies, type Cadence, type Category, type Currency, type Subscription } from "./schema";
  import CategoryMark from "./CategoryMark.svelte";

  const currencyItems = currencies.map((value) => ({ value, label: value }));
  const cadenceItems = [
    { value: "monthly", label: "Monthly" },
    { value: "annual", label: "Annual" },
  ] as const;
  const categoryItems = categories.map((value) => ({ value, label: value }));

  type Draft = {
    name: string;
    amount: number;
    cadence: Cadence;
    nextRenewal: string;
    category: Category;
    note: string;
    active: boolean;
  };

  function localDate(date = new Date()): string {
    return new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 10);
  }

  const doc = useDocument(schema);
  let view = $state<"active" | "all">("active");
  let showDialog = $state(false);
  let editingId = $state<string | null>(null);
  let draft = $state<Draft>(blankSubscription());

  const activeSubscriptions = $derived(doc.current.subscriptions.filter((item) => item.active));
  const visibleSubscriptions = $derived(
    [...doc.current.subscriptions]
      .filter((item) => view === "all" || item.active)
      .sort((a, b) => Number(b.active) - Number(a.active) || a.nextRenewal.localeCompare(b.nextRenewal) || a.name.localeCompare(b.name)),
  );
  const monthlyPace = $derived(activeSubscriptions.reduce((total, item) => total + monthlyAmount(item), 0));

  function blankSubscription(): Draft {
    return { name: "", amount: 0, cadence: "monthly", nextRenewal: localDate(), category: "Entertainment", note: "", active: true };
  }
  function monthlyAmount(item: Pick<Subscription, "cadence" | "amount">): number {
    return item.cadence === "annual" ? item.amount / 12 : item.amount;
  }
  function money(value: number): string {
    return new Intl.NumberFormat(undefined, { style: "currency", currency: doc.current.currency }).format(value);
  }
  function dateLabel(value: string): string {
    if (!value || Number.isNaN(new Date(`${value}T12:00:00`).getTime())) return "No date";
    return new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", year: "numeric" }).format(new Date(`${value}T12:00:00`));
  }
  function renewalState(value: string): "overdue" | "soon" | "normal" {
    if (!value || Number.isNaN(new Date(`${value}T12:00:00`).getTime())) return "normal";
    const today = localDate();
    if (value < today) return "overdue";
    const soon = new Date(`${today}T12:00:00`);
    soon.setDate(soon.getDate() + 30);
    return value <= localDate(soon) ? "soon" : "normal";
  }
  function cadenceLabel(cadence: string): string {
    return cadence === "annual" ? "yr" : "mo";
  }
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
    draft = blankSubscription();
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
      active: item.active,
    };
    showDialog = true;
  }
  function closeDialog(): void {
    showDialog = false;
  }
  function save(): void {
    const name = draft.name.trim();
    const amount = Number(draft.amount);
    if (!name || !Number.isFinite(amount) || amount <= 0 || !draft.nextRenewal) return;
    const note = draft.note.trim();
    if (editingId) {
      const item = doc.current.subscriptions.find((entry) => entry.$id === editingId);
      if (!item) return;
      doc.change((tx) => {
        const row = tx.at(item);
        row.name.replace(name);
        row.amount.set(amount);
        row.cadence.set(draft.cadence);
        row.nextRenewal.set(draft.nextRenewal);
        row.category.set(draft.category);
        row.note.replace(note);
      });
    } else {
      doc.fields.subscriptions.insert({
        name,
        amount,
        cadence: draft.cadence,
        nextRenewal: draft.nextRenewal,
        category: draft.category,
        note,
        active: true,
      });
    }
    closeDialog();
  }
  function toggle(item: Subscription): void {
    doc.at(item).active.set(!item.active);
  }
  function remove(): void {
    if (!editingId) return;
    doc.fields.subscriptions.remove(editingId);
    closeDialog();
  }
</script>

{#snippet serviceRows(items: readonly Subscription[], editable: boolean)}
  <ol class="serviceList">
    {#each items as item (item.$id)}
      <li class="row" data-paused={!item.active}>
        {#if editable}
          <button class="serviceCopy" onclick={() => beginEdit(item)} aria-label={`Edit ${item.name}`}>
            <CategoryMark category={item.category} /><span class="serviceText"><strong>{item.name}</strong>
            <small>{item.category}{item.note ? ` · ${item.note}` : ""}</small></span>
          </button>
        {:else}
          <div class="serviceCopy">
            <CategoryMark category={item.category} /><span class="serviceText"><strong>{item.name}</strong>
            <small>{item.category}{item.note ? ` · ${item.note}` : ""}</small></span>
          </div>
        {/if}
        <time data-state={editable && item.active ? renewalState(item.nextRenewal) : "normal"}>
          <span>{!item.active ? "Paused" : editable && renewalState(item.nextRenewal) === "overdue" ? "Date passed" : "Renews"}</span>
          <b>{dateLabel(item.nextRenewal)}</b>
        </time>
        <output>
          <strong>{money(item.amount)}</strong>
          <small>/{cadenceLabel(item.cadence)}</small>
        </output>
        {#if editable}
          <button class="toggle" data-slop-export="hide" onclick={() => toggle(item)} title="Changes tracking only, not your subscription" aria-label={item.active ? `Pause tracking ${item.name}` : `Resume tracking ${item.name}`}>
            {#if item.active}<Pause size={12} strokeWidth={1.8} />{:else}<Play size={12} strokeWidth={1.8} />{/if}
          </button>
        {/if}
      </li>
    {/each}
  </ol>
{/snippet}

<Slop>
  <main class="canvas" data-slop-selection="none">
    <article class="ledger" aria-label="Subscription ledger">
      <header class="masthead">
        <div>
          <h1>Subscriptions<span class="headingLoop" aria-hidden="true"><Repeat2 size={24} /></span></h1>
        </div>
        <div class="currencyField">
          <span>Currency</span>
          <Select.Root type="single" value={doc.current.currency} items={currencyItems} onValueChange={(value) => { if (isCurrency(value)) doc.fields.currency.set(value); }}>
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
          <strong>{money(monthlyPace)}</strong>
        </div>
        <div>
          <span>Yearly estimate</span>
          <b>{money(monthlyPace * 12)}</b>
        </div>
        <div class="totalNote">
          <CalendarDays size={15} strokeWidth={1.8} />
          <span>{activeSubscriptions.length} active service{activeSubscriptions.length === 1 ? "" : "s"}</span>
        </div>
      </section>

      <section class="services" aria-labelledby="services-title">
        <div class="listHead">
          <Tabs.Root value={view} onValueChange={(value) => { if (value === "active" || value === "all") view = value; }}>
            <Tabs.List class="viewTabs" data-slop-export="hide" aria-label="Subscription view">
              <Tabs.Trigger value="active" class="viewTab">Active</Tabs.Trigger>
              <Tabs.Trigger value="all" class="viewTab">All</Tabs.Trigger>
            </Tabs.List>
          </Tabs.Root>
          <h2 id="services-title">{view === "active" ? "Active services" : "All services"}</h2>
          <Button.Root class="add" data-slop-export="hide" onclick={beginAdd}><Plus size={14} strokeWidth={1.8} /> Add service</Button.Root>
        </div>

        {#if visibleSubscriptions.length}
          {@render serviceRows(visibleSubscriptions, true)}
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
        <span>{activeSubscriptions.length} active service{activeSubscriptions.length === 1 ? "" : "s"}</span>
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

  {#snippet exportView()}
    {@const active = doc.current.subscriptions.filter((item) => item.active)}
    {@const visible = [...doc.current.subscriptions]
      .filter((item) => view === "all" || item.active)
      .sort((a, b) => Number(b.active) - Number(a.active) || a.nextRenewal.localeCompare(b.nextRenewal) || a.name.localeCompare(b.name))}
    {@const pace = active.reduce((total, item) => total + monthlyAmount(item), 0)}
    <article class="exportLedger" aria-label="Exported subscription ledger">
      <header class="masthead">
        <div>
          <h1>Subscriptions<span class="headingLoop" aria-hidden="true"><Repeat2 size={24} /></span></h1>
        </div>
        <div class="currencyField">
          <span>Currency</span>
          <strong class="currencyTrigger">{doc.current.currency}</strong>
        </div>
      </header>
      <section class="totals" aria-label="Active subscription totals">
        <div><span>Monthly total</span><strong>{money(pace)}</strong></div>
        <div><span>Yearly estimate</span><b>{money(pace * 12)}</b></div>
        <div class="totalNote"><span>{active.length} active service{active.length === 1 ? "" : "s"}</span></div>
      </section>
      <section class="services" aria-label={view === "active" ? "Active services" : "All services"}>
        <div class="listHead">
          <h2>{view === "active" ? "Active services" : "All services"}</h2>
        </div>
        {#if visible.length}
          {@render serviceRows(visible, false)}
        {:else}
          <div class="empty">
            <strong>No recurring costs yet.</strong>
            <p>Add a service you want to keep an eye on.</p>
          </div>
        {/if}
      </section>
      <footer class="footer">
        <span>{active.length} active service{active.length === 1 ? "" : "s"}</span>
        <span>{doc.current.subscriptions.length} on the ledger</span>
      </footer>
    </article>
  {/snippet}

  {#snippet icon()}
    <div class="iconSurface" aria-hidden="true">
      <svg class="iconGraphic" viewBox="0 0 512 512" fill="none">
        <rect x="28" y="28" width="456" height="456" rx="104" fill="var(--slop-accentInk)"/>
        <rect x="145" y="135" width="242" height="224" rx="30" transform="rotate(9 145 135)" fill="var(--slop-accent)"/>
        <rect x="111" y="152" width="242" height="224" rx="30" fill="var(--slop-paper)"/>
        <path d="M144 206H320" stroke="var(--slop-paperSoft)" stroke-width="25"/>
        <path d="M153 266H242M153 314H206" stroke="var(--slop-accentInk)" stroke-width="16" stroke-linecap="round"/>
        <path d="M90 184A178 178 0 0 1 407 153M407 153V92M407 153H348M422 330A178 178 0 0 1 105 361M105 361V422M105 361H164" stroke="var(--slop-mint)" stroke-width="23" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
    </div>
  {/snippet}
</Slop>
