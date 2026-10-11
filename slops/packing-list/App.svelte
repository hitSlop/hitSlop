<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { bindText } from "hitslop/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { Tabs, Checkbox, Select, Button, Tooltip, Dialog, Progress, Popover } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import X from "@lucide/svelte/icons/x";
  import doc from "./schema";
  import { PRESETS } from "./presets";
  import { addItem, applyPreset, clearPacked, cycleQuantity, packingView, removeItem, setAllPacked, setEssential, setFlag, setPacked } from "./items";

  type Category = (typeof doc.current.categories)[number];

  const QTY_ITEMS = [1, 2, 3, 4, 5, 6, 7, 8, 9].map((value) => ({ value: String(value), label: `×${value}` }));
  const POPULAR_FLAGS = [
    "🇯🇵", "🇫🇷", "🇮🇹", "🇪🇸", "🇬🇧", "🇺🇸", "🇩🇪", "🇨🇦",
    "🇲🇽", "🇨🇭", "🇬🇷", "🇵🇹", "🇳🇱", "🇹🇭", "🇰🇷", "🇦🇺",
    "🇳🇿", "🇮🇸", "🇳🇴", "🇸🇪", "🇧🇷", "🇦🇷", "🇮🇩", "🇻🇳",
    "🇸🇬", "🇲🇾", "🇿🇦", "🇪🇬", "🇲🇦", "🇹🇷", "🇮🇪", "🇦🇹",
    "✈️", "🌴", "🏔️", "🎒",
  ];

  // What the person is looking at. Local to this page; the export always shows every item.
  let filter = $state("all");
  let draftText = $state("");
  let aisle = $state("carry");
  let qty = $state("1");
  let composer = $state<HTMLInputElement>();
  let presetsOpen = $state(false);
  let flagOpen = $state(false);
  let customFlag = $state("");

  const view = $derived(packingView(doc.current, filter));
  const categoryItems = $derived(doc.current.categories.map((category) => ({ value: category.key, label: category.name })));
  const flipMs = $derived(prefersReducedMotion.current ? 0 : 220);

  function isCategory(value: string): boolean {
    return doc.current.categories.some((category) => category.key === value);
  }
  function isFilter(value: string): boolean {
    return value === "all" || value === "remaining" || isCategory(value);
  }
  function isQty(value: string): boolean {
    return QTY_ITEMS.some((item) => item.value === value);
  }
  function submitItem(event: SubmitEvent) {
    event.preventDefault();
    const text = draftText.trim();
    if (!text) return;
    doc.change((tx) => addItem(tx, doc.current, { text, category: aisle, quantity: Number(qty) }));
    draftText = "";
    composer?.focus();
  }
  function applyCustomFlag() {
    const trimmed = customFlag.trim();
    if (!trimmed) return;
    doc.change((tx) => setFlag(tx, trimmed));
    customFlag = "";
    flagOpen = false;
  }
  function selectFlag(flag: string) {
    doc.change((tx) => setFlag(tx, flag));
    flagOpen = false;
  }
  function choosePreset(id: string) {
    doc.change((tx) => applyPreset(tx, doc.current, id));
    presetsOpen = false;
  }
  function stampFor(category: Category): string {
    return category.color || "slate";
  }
</script>

  <Tooltip.Provider>
    <main class="tag" aria-label="Packing list">
      <div class="airmail" aria-hidden="true"></div>

      <header class="header">
        <div class="eyelet-row">
          <Popover.Root bind:open={flagOpen}>
            <Popover.Trigger class="flag-btn" aria-label="Change destination flag (current: {doc.current.flag || '🇯🇵'})">
              <span class="flag-emoji">{doc.current.flag || "🇯🇵"}</span>
            </Popover.Trigger>
            <Popover.Portal>
              <Popover.Content class="flag-popover" sideOffset={8} data-slop-export="hide">
                <div class="flag-head">
                  <span>DESTINATION FLAG</span>
                  <Popover.Close class="flag-close" aria-label="Close flag picker">✕</Popover.Close>
                </div>
                <div class="flag-grid">
                  {#each POPULAR_FLAGS as flag}
                    <Popover.Close class="flag-opt" data-selected={doc.current.flag === flag} aria-label="Select flag {flag}" onclick={() => selectFlag(flag)}>{flag}</Popover.Close>
                  {/each}
                </div>
                <form class="custom-flag" onsubmit={(event) => { event.preventDefault(); applyCustomFlag(); }}>
                  <input class="custom-flag-input" placeholder="Paste emoji / flag..." aria-label="Custom flag or emoji" bind:value={customFlag} />
                  <Button.Root class="apply-flag" type="submit">Set</Button.Root>
                </form>
              </Popover.Content>
            </Popover.Portal>
          </Popover.Root>

          <span class="airline">HS-BAG // TRAVEL MANIFEST</span>
          <Button.Root class="pill" data-slop-export="hide" onclick={() => (presetsOpen = true)}>
            <Sparkles size={11} />
            Presets
          </Button.Root>
        </div>

        <div class="meta">
          <div class="field">
            <label for="trip-title-input" class="meta-label">TRIP / EXPEDITION</label>
            <input id="trip-title-input" class="meta-input title-input" aria-label="Trip expedition title" use:bindText={doc.fields.tripTitle} />
          </div>
          <div class="submeta">
            <div class="field">
              <label for="dest-code-input" class="meta-label">DESTINATION</label>
              <input id="dest-code-input" class="meta-input dest-input" aria-label="Destination code" use:bindText={doc.fields.destination} />
            </div>
            <div class="field">
              <label for="traveler-input" class="meta-label">PASSENGER</label>
              <input id="traveler-input" class="meta-input passenger-input" aria-label="Passenger name" use:bindText={doc.fields.traveler} />
            </div>
            <div class="field date-field">
              <label for="date-input" class="meta-label">DEPARTURE</label>
              <input id="date-input" type="date" class="meta-input date-input" aria-label="Departure date" bind:value={doc.fields.departureDate.value} />
            </div>
          </div>
        </div>
      </header>

      <section class="readiness">
        <div class="readiness-row">
          <div class="readiness-status">
            <span>BAGGAGE STATUS:</span>
            <span class="ready-badge" data-complete={view.complete}>{view.packed} / {view.total} PACKED ({view.percent}%)</span>
          </div>
          <div class="quick-actions" data-slop-export="hide">
            <Button.Root class="pill" onclick={() => doc.change((tx) => setAllPacked(tx, doc.current, true))} disabled={!view.remaining}>Pack All</Button.Root>
            <Button.Root class="pill" onclick={() => doc.change((tx) => setAllPacked(tx, doc.current, false))} disabled={!view.packed}>Reset</Button.Root>
            <Button.Root class="pill" onclick={() => doc.change((tx) => clearPacked(tx, doc.current))} disabled={!view.packed}>Archive packed{view.packed ? ` (${view.packed})` : ""}</Button.Root>
          </div>
        </div>
        <Progress.Root class="track" value={view.packed} max={view.total || 1} aria-label="Packing readiness">
          <div class="fill" data-complete={view.complete} style:width="{view.percent}%"></div>
        </Progress.Root>
      </section>

      <Tabs.Root value={filter} onValueChange={(value) => { if (isFilter(value)) filter = value; }}>
        <Tabs.List class="tabs" aria-label="Category filters" data-slop-export="hide">
          <Tabs.Trigger value="all" class="tab">All <span class="pill-count">({view.total})</span></Tabs.Trigger>
          <Tabs.Trigger value="remaining" class="tab">Remaining <span class="pill-count">({view.remaining})</span></Tabs.Trigger>
          {#each view.categories as row (row.category.$id)}
            <Tabs.Trigger value={row.category.key} class="tab">{row.category.tagCode} <span class="pill-count">({row.packed}/{row.total})</span></Tabs.Trigger>
          {/each}
        </Tabs.List>
      </Tabs.Root>

      <form class="composer" data-slop-export="hide" onsubmit={submitItem}>
        <input bind:this={composer} class="add-input" bind:value={draftText} aria-label="New packing item" placeholder="Add passport, charger…" />
        <Select.Root type="single" value={qty} items={QTY_ITEMS} onValueChange={(value) => { if (isQty(value)) qty = value; }}>
          <Select.Trigger class="select-trigger" aria-label="Item quantity">
            <Select.Value placeholder="×1" />
            <ChevronDown size={12} strokeWidth={2.2} data-slop-export="hide" />
          </Select.Trigger>
          <Select.Portal>
            <Select.Content class="select-content" sideOffset={6}>
              <Select.Viewport>
                {#each QTY_ITEMS as item (item.value)}
                  <Select.Item value={item.value} label={item.label}>
                    {#snippet children({ selected })}
                      {item.label}{#if selected}<Check size={12} strokeWidth={2.4} />{/if}
                    {/snippet}
                  </Select.Item>
                {/each}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>
        <Select.Root type="single" value={aisle} items={categoryItems} onValueChange={(value) => { if (isCategory(value)) aisle = value; }}>
          <Select.Trigger class="select-trigger" aria-label="Bag category for new item">
            <Select.Value placeholder="Category" />
            <ChevronDown size={12} strokeWidth={2.2} data-slop-export="hide" />
          </Select.Trigger>
          <Select.Portal>
            <Select.Content class="select-content" sideOffset={6}>
              <Select.Viewport>
                {#each categoryItems as item (item.value)}
                  <Select.Item value={item.value} label={item.label}>
                    {#snippet children({ selected })}
                      {item.label}{#if selected}<Check size={12} strokeWidth={2.4} />{/if}
                    {/snippet}
                  </Select.Item>
                {/each}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>
        <Button.Root class="add" type="submit" aria-label="Add item" disabled={!draftText.trim()}><Plus size={14} /><span>Add</span></Button.Root>
      </form>

      <div class="checklist">
        {#each view.sections as section (section.category.$id)}
          {#if section.items.length > 0}
            <section class="block" aria-label={section.category.name}>
              <div class="block-head">
                <span class="stamp" data-stamp={stampFor(section.category)}>{section.category.tagCode} // {section.category.name}</span>
                <span class="block-count">{section.packed} of {section.total} packed</span>
              </div>
              <ul class="list">
                {#each section.items as item (item.$id)}
                  <li class="row" data-packed={item.packed} animate:flip={{ duration: flipMs }}>
                    <Checkbox.Root checked={item.packed} onCheckedChange={(checked) => doc.change((tx) => setPacked(tx, item.$id, checked === true))} aria-label="Toggle packed status for {item.text}">
                      {#snippet children({ checked })}{#if checked}<span class="check-icon">✓</span>{/if}{/snippet}
                    </Checkbox.Root>
                    <button type="button" class="qty" title="Click to cycle quantity" onclick={() => doc.change((tx) => cycleQuantity(tx, item.$id, item.quantity))}>×{item.quantity}</button>
                    <input class="item-text" aria-label="Item description" use:bindText={doc.at(item).text} />
                    <button type="button" class="star" data-active={item.essential} title="Toggle essential item" onclick={() => doc.change((tx) => setEssential(tx, item.$id, !item.essential))}>{item.essential ? "★" : "☆"}</button>
                    <Tooltip.Root>
                      <Tooltip.Trigger class="remove" data-slop-export="hide" aria-label="Delete {item.text || 'untitled item'}" onclick={() => doc.change((tx) => removeItem(tx, item.$id))}><Trash2 size={13} /></Tooltip.Trigger>
                      <Tooltip.Portal>
                        <Tooltip.Content class="tooltip" sideOffset={6}>Remove from the tag</Tooltip.Content>
                      </Tooltip.Portal>
                    </Tooltip.Root>
                  </li>
                {/each}
              </ul>
            </section>
          {/if}
        {/each}

        {#if view.empty}
          <div class="empty">
            <h2>{view.empty.title}</h2>
            <p>{view.empty.body}</p>
          </div>
        {/if}
      </div>

      <footer class="footer">
        <div class="barcode-block">
          <div class="barcode" aria-hidden="true"></div>
          <span class="barcode-code">{doc.current.bagTag} // CHECKED AIRLINE TAG</span>
        </div>
        <div class="footer-stamp" data-complete={view.complete}>{view.complete ? "100% READY FOR FLIGHT" : "PACK IN PROGRESS"}</div>
      </footer>

      <Dialog.Root bind:open={presetsOpen}>
        <Dialog.Portal>
          <Dialog.Overlay class="overlay" data-slop-export="hide" />
          <Dialog.Content class="presets-card" aria-labelledby="preset-modal-title" data-slop-export="hide">
            <div class="presets-head">
              <Dialog.Title id="preset-modal-title">TRAVEL PACKING PRESETS</Dialog.Title>
              <Dialog.Close class="pill" aria-label="Close presets"><X size={14} /></Dialog.Close>
            </div>
            <div class="preset-list">
              {#each PRESETS as preset (preset.id)}
                <button type="button" class="preset-item" onclick={() => choosePreset(preset.id)}>
                  <span class="preset-info">
                    <span class="preset-name">{preset.name}</span>
                    <span class="preset-desc">{preset.description} ({preset.items.length} essentials)</span>
                  </span>
                  <span class="preset-add">+ Add Items</span>
                </button>
              {/each}
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </main>
  </Tooltip.Provider>
