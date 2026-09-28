<script lang="ts">
  import { Slop, bindText, bindValue, useDocument } from "@hitslop/document/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { Tabs, Checkbox, Select, Button, Tooltip, Dialog, Progress, Popover } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import X from "@lucide/svelte/icons/x";
  import schema, { type PackingCategory, type PackingItem } from "./schema";
  import { PRESETS, type Preset } from "./presets";

  const QTY_ITEMS = [1, 2, 3, 4, 5, 6, 7, 8, 9].map((value) => ({ value: String(value), label: `×${value}` }));
  const POPULAR_FLAGS = [
    "🇯🇵", "🇫🇷", "🇮🇹", "🇪🇸", "🇬🇧", "🇺🇸", "🇩🇪", "🇨🇦",
    "🇲🇽", "🇨🇭", "🇬🇷", "🇵🇹", "🇳🇱", "🇹🇭", "🇰🇷", "🇦🇺",
    "🇳🇿", "🇮🇸", "🇳🇴", "🇸🇪", "🇧🇷", "🇦🇷", "🇮🇩", "🇻🇳",
    "🇸🇬", "🇲🇾", "🇿🇦", "🇪🇬", "🇲🇦", "🇹🇷", "🇮🇪", "🇦🇹",
    "✈️", "🌴", "🏔️", "🎒",
  ];
  const iconRows = [
    { stamp: "DOCS", tone: "cobalt", length: "long" },
    { stamp: "CARRY", tone: "vermilion", length: "med" },
    { stamp: "TECH", tone: "emerald", length: "long" },
    { stamp: "TOILET", tone: "amber", length: "short" },
  ] as const;

  const doc = useDocument(schema);

  let filter = $state("all");
  let draft = $state("");
  let aisle = $state("carry");
  let qty = $state("1");
  let composer = $state<HTMLInputElement>();
  let presetsOpen = $state(false);
  let flagOpen = $state(false);
  let customFlag = $state("");

  const categoryItems = $derived(doc.current.categories.map((category) => ({ value: category.key, label: category.name })));
  const totalCount = $derived(doc.current.items.length);
  const packedCount = $derived(doc.current.items.filter((item) => item.packed).length);
  const packedPercent = $derived(totalCount > 0 ? Math.round((packedCount / totalCount) * 100) : 0);
  const fullyPacked = $derived(totalCount > 0 && packedCount === totalCount);
  const remainingCount = $derived(totalCount - packedCount);
  const visibleCategories = $derived(
    filter === "all" || filter === "remaining"
      ? doc.current.categories
      : doc.current.categories.filter((category) => category.key === filter),
  );
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
  function countsFor(key: string): { packed: number; total: number } {
    const items = doc.current.items.filter((item) => item.key === key);
    return { total: items.length, packed: items.filter((item) => item.packed).length };
  }
  function itemsFor(key: string): PackingItem[] {
    return doc.current.items.filter((item) => {
      if (item.key !== key) return false;
      if (filter === "remaining") return !item.packed;
      return true;
    });
  }
  function addItem() {
    const text = draft.trim();
    const quantity = Number(qty);
    if (!text || !Number.isInteger(quantity)) return;
    doc.fields.items.insert({ key: aisle, text, quantity, packed: false, essential: false });
    draft = "";
    composer?.focus();
  }
  function removeItem(id: string) {
    doc.fields.items.remove(id);
  }
  function cycleQty(item: PackingItem) {
    doc.at(item).quantity.set(item.quantity >= 9 ? 1 : item.quantity + 1);
  }
  function packAll() {
    const pending = doc.current.items.filter((item) => !item.packed);
    if (!pending.length) return;
    doc.change((tx) => {
      for (const item of pending) tx.at(item).packed.set(true);
    });
  }
  function unpackAll() {
    const packed = doc.current.items.filter((item) => item.packed);
    if (!packed.length) return;
    doc.change((tx) => {
      for (const item of packed) tx.at(item).packed.set(false);
    });
  }
  function clearPacked() {
    const packed = doc.current.items.filter((item) => item.packed);
    if (!packed.length) return;
    doc.change((tx) => {
      for (const item of packed) tx.fields.items.remove(item.$id);
    });
  }
  function applyPreset(preset: Preset) {
    const existing = new Set(doc.current.items.map((item) => item.text.toLowerCase()));
    const next = preset.items.filter((item) => !existing.has(item.text.toLowerCase()));
    if (next.length) {
      doc.change((tx) => {
        for (const item of next) {
          tx.fields.items.insert({
            key: item.key,
            text: item.text,
            quantity: item.quantity,
            packed: false,
            essential: Boolean(item.essential),
          });
        }
      });
    }
    presetsOpen = false;
  }
  function selectFlag(flag: string) {
    doc.fields.flag.set(flag);
    flagOpen = false;
  }
  function applyCustomFlag() {
    const trimmed = customFlag.trim();
    if (!trimmed) return;
    doc.fields.flag.set(trimmed);
    customFlag = "";
    flagOpen = false;
  }
  function stampFor(category: PackingCategory): string {
    return category.color || "slate";
  }
  function emptyCopy(): { title: string; body: string } {
    if (totalCount === 0) return { title: "The tag is empty.", body: "Add a passport, charger, or whatever this trip needs." };
    if (filter === "remaining") return { title: "Everything is packed.", body: "Reset the tag for the next trip, or archive packed items." };
    const category = doc.current.categories.find((item) => item.key === filter);
    return { title: `Nothing in ${category?.name ?? "this bag"} yet.`, body: "Add an item to this category, or switch back to All." };
  }
  function exportCategories(): readonly PackingCategory[] {
    return filter === "all" || filter === "remaining"
      ? doc.current.categories
      : doc.current.categories.filter((category) => category.key === filter);
  }
</script>

<Slop>
  <Tooltip.Provider>
    <main class="tag" data-slop-selection="none" aria-label="Packing list">
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
              <input id="date-input" type="date" class="meta-input date-input" aria-label="Departure date" use:bindValue={doc.fields.departureDate} />
            </div>
          </div>
        </div>
      </header>

      <section class="readiness">
        <div class="readiness-row">
          <div class="readiness-status">
            <span>BAGGAGE STATUS:</span>
            <span class="ready-badge" data-complete={fullyPacked}>{packedCount} / {totalCount} PACKED ({packedPercent}%)</span>
          </div>
          <div class="quick-actions" data-slop-export="hide">
            <Button.Root class="pill" onclick={packAll} disabled={!remainingCount}>Pack All</Button.Root>
            <Button.Root class="pill" onclick={unpackAll} disabled={!packedCount}>Reset</Button.Root>
            <Button.Root class="pill" onclick={clearPacked} disabled={!packedCount}>Archive packed{packedCount ? ` (${packedCount})` : ""}</Button.Root>
          </div>
        </div>
        <Progress.Root class="track" value={packedCount} max={totalCount || 1} aria-label="Packing readiness">
          <div class="fill" data-complete={fullyPacked} style:width="{packedPercent}%"></div>
        </Progress.Root>
      </section>

      <Tabs.Root value={filter} onValueChange={(value) => { if (isFilter(value)) filter = value; }}>
        <Tabs.List class="tabs" aria-label="Category filters" data-slop-export="hide">
          <Tabs.Trigger value="all" class="tab">All <span class="pill-count">({totalCount})</span></Tabs.Trigger>
          <Tabs.Trigger value="remaining" class="tab">Remaining <span class="pill-count">({remainingCount})</span></Tabs.Trigger>
          {#each doc.current.categories as category (category.$id)}
            {@const counts = countsFor(category.key)}
            <Tabs.Trigger value={category.key} class="tab">{category.tagCode} <span class="pill-count">({counts.packed}/{counts.total})</span></Tabs.Trigger>
          {/each}
        </Tabs.List>
      </Tabs.Root>

      <form class="composer" data-slop-export="hide" onsubmit={(event) => { event.preventDefault(); addItem(); }}>
        <input bind:this={composer} class="add-input" bind:value={draft} aria-label="New packing item" placeholder="Add passport, charger…" />
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
        <Button.Root class="add" type="submit" aria-label="Add item" disabled={!draft.trim()}><Plus size={14} /><span>Add</span></Button.Root>
      </form>

      <div class="checklist">
        {#each visibleCategories as category (category.$id)}
          {@const items = itemsFor(category.key)}
          {@const counts = countsFor(category.key)}
          {#if items.length > 0}
            <section class="block" aria-label={category.name}>
              <div class="block-head">
                <span class="stamp" data-stamp={stampFor(category)}>{category.tagCode} // {category.name}</span>
                <span class="block-count">{counts.packed} of {counts.total} packed</span>
              </div>
              <ul class="list">
                {#each items as item (item.$id)}
                  <li class="row" data-packed={item.packed} animate:flip={{ duration: flipMs }}>
                    <Checkbox.Root checked={item.packed} onCheckedChange={(checked) => doc.at(item).packed.set(checked === true)} aria-label="Toggle packed status for {item.text}">
                      {#snippet children({ checked })}{#if checked}<span class="check-icon">✓</span>{/if}{/snippet}
                    </Checkbox.Root>
                    <button type="button" class="qty" title="Click to cycle quantity" onclick={() => cycleQty(item)}>×{item.quantity}</button>
                    <input class="item-text" aria-label="Item description" use:bindText={doc.at(item).text} />
                    <button type="button" class="star" data-active={item.essential} title="Toggle essential item" onclick={() => doc.at(item).essential.set(!item.essential)}>{item.essential ? "★" : "☆"}</button>
                    <Tooltip.Root>
                      <Tooltip.Trigger class="remove" data-slop-export="hide" aria-label="Delete {item.text || 'untitled item'}" onclick={() => removeItem(item.$id)}><Trash2 size={13} /></Tooltip.Trigger>
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

        {#if visibleCategories.every((category) => itemsFor(category.key).length === 0)}
          {@const copy = emptyCopy()}
          <div class="empty">
            <h2>{copy.title}</h2>
            <p>{copy.body}</p>
          </div>
        {/if}
      </div>

      <footer class="footer">
        <div class="barcode-block">
          <div class="barcode" aria-hidden="true"></div>
          <span class="barcode-code">{doc.current.bagTag} // CHECKED AIRLINE TAG</span>
        </div>
        <div class="footer-stamp" data-complete={fullyPacked}>{fullyPacked ? "100% READY FOR FLIGHT" : "PACK IN PROGRESS"}</div>
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
                <button type="button" class="preset-item" onclick={() => applyPreset(preset)}>
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

  {#snippet exportView()}
    {@const categories = exportCategories()}
    {@const packed = doc.current.items.filter((item) => item.packed).length}
    {@const total = doc.current.items.length}
    {@const percent = total > 0 ? Math.round((packed / total) * 100) : 0}
    {@const complete = total > 0 && packed === total}
    <article class="export-tag" aria-label="Exported packing list">
      <div class="airmail" aria-hidden="true"></div>
      <header class="header">
        <div class="eyelet-row">
          <span class="flag-btn" aria-hidden="true"><span class="flag-emoji">{doc.current.flag || "🇯🇵"}</span></span>
          <span class="airline">HS-BAG // TRAVEL MANIFEST</span>
        </div>
        <div class="meta">
          <div class="field">
            <span class="meta-label">TRIP / EXPEDITION</span>
            <strong class="meta-input title-input">{doc.current.tripTitle || "Untitled trip"}</strong>
          </div>
          <div class="submeta">
            <div class="field">
              <span class="meta-label">DESTINATION</span>
              <strong class="meta-input dest-input">{doc.current.destination}</strong>
            </div>
            <div class="field">
              <span class="meta-label">PASSENGER</span>
              <strong class="meta-input passenger-input">{doc.current.traveler}</strong>
            </div>
            <div class="field">
              <span class="meta-label">DEPARTURE</span>
              <strong class="meta-input date-input">{doc.current.departureDate}</strong>
            </div>
          </div>
        </div>
      </header>
      <section class="readiness">
        <div class="readiness-row">
          <div class="readiness-status">
            <span>BAGGAGE STATUS:</span>
            <span class="ready-badge" data-complete={complete}>{packed} / {total} PACKED ({percent}%)</span>
          </div>
        </div>
        <div class="track"><div class="fill" data-complete={complete} style:width="{percent}%"></div></div>
      </section>
      <div class="export-list">
        {#each categories as category (category.$id)}
          {@const items = itemsFor(category.key)}
          {@const counts = countsFor(category.key)}
          {#if items.length > 0}
            <section class="block" aria-label={category.name}>
              <div class="block-head">
                <span class="stamp" data-stamp={category.color}>{category.tagCode} // {category.name}</span>
                <span class="block-count">{counts.packed} of {counts.total} packed</span>
              </div>
              <ul class="list">
                {#each items as item (item.$id)}
                  <li class="row" data-packed={item.packed}>
                    <span data-checkbox-root data-state={item.packed ? "checked" : "unchecked"}>{#if item.packed}<span class="check-icon">✓</span>{/if}</span>
                    <span class="qty">×{item.quantity}</span>
                    <span class="item-text">{item.text || "Untitled item"}</span>
                    <span class="star" data-active={item.essential}>{item.essential ? "★" : ""}</span>
                  </li>
                {/each}
              </ul>
            </section>
          {/if}
        {:else}
          <div class="empty"><h2>Nothing to pack yet.</h2></div>
        {/each}
      </div>
      <footer class="footer">
        <div class="barcode-block">
          <div class="barcode" aria-hidden="true"></div>
          <span class="barcode-code">{doc.current.bagTag} // CHECKED AIRLINE TAG</span>
        </div>
        <div class="footer-stamp" data-complete={complete}>{complete ? "100% READY FOR FLIGHT" : "PACK IN PROGRESS"}</div>
      </footer>
    </article>
  {/snippet}

  {#snippet icon()}
    {@const marks = totalCount > 0 ? Math.round(4 * packedCount / totalCount) : 0}
    <div class="icon-surface" aria-hidden="true">
      <div class="icon-tag">
        <div class="icon-grommet">
          <div class="icon-loop"></div>
          <div class="icon-eyelet"><div class="icon-hole"></div></div>
        </div>
        <div class="icon-airmail"></div>
        <div class="icon-head">
          <div class="icon-flight">HS-AIR</div>
          <div class="icon-dest">HND</div>
        </div>
        <div class="icon-checks">
          {#each iconRows as row, index}
            <div class="icon-row">
              <span class="icon-box" data-done={index < marks}>{#if index < marks}✓{/if}</span>
              <span class="icon-line" data-length={row.length}></span>
              <span class="icon-stamp" data-stamp={row.tone}>{row.stamp}</span>
            </div>
          {/each}
        </div>
        <div class="icon-seal">
          <span class="icon-seal-text">{packedCount > 0 && packedCount === totalCount ? "PACKED" : "PACKING"}</span>
          <span class="icon-seal-sub">{totalCount > 0 ? `${Math.round(100 * packedCount / totalCount)}% READY` : "EMPTY TAG"}</span>
        </div>
        <div class="icon-barcode-row">
          <div class="icon-barcode"></div>
          <span class="icon-tag-num">#HS-9402</span>
        </div>
      </div>
    </div>
  {/snippet}
</Slop>
