<script lang="ts">
  import { onMount, tick } from "svelte";
  import { Popover } from "bits-ui";
  import { Slop, useDocument, bindText } from "@hitslop/document/svelte";
  import Download from "@lucide/svelte/icons/download";
  import Eraser from "@lucide/svelte/icons/eraser";
  import schema, { stamps, tints, type Tint } from "./schema";
  import { allKeys, cellKey, columnName, COLUMNS, csv, display, evaluate, isError, ROWS, type Value } from "./formula";

  const doc = useDocument(schema);
  const DEFAULT_WIDTH = 96;
  type Point = { column: number; row: number };

  const inputs = $derived(Object.fromEntries(Object.entries(doc.current.cells).map(([key, cell]) => [key, cell.input])));
  const results = $derived(evaluate(inputs));
  let anchor = $state<Point>({ column: 1, row: 8 });
  let cursor = $state<Point>({ column: 1, row: 8 });
  let editing = $state<{ key: string; from: "cell" | "bar" } | null>(null);
  let draft = $state("");
  let dragging = $state(false);
  let resizing = $state<{ letter: string; width: number } | null>(null);
  let bouncing = $state<ReadonlySet<string>>(new Set());
  let notice = $state("");
  let stampOpen = $state(false);
  let grid: HTMLElement;

  const activeKey = $derived(cellKey(cursor.column, cursor.row));
  const bounds = $derived({
    left: Math.min(anchor.column, cursor.column), right: Math.max(anchor.column, cursor.column),
    top: Math.min(anchor.row, cursor.row), bottom: Math.max(anchor.row, cursor.row),
  });
  const selection = $derived(allKeys.filter((_, i) => {
    const row = Math.floor(i / COLUMNS), column = i % COLUMNS;
    return column >= bounds.left && column <= bounds.right && row >= bounds.top && row <= bounds.bottom;
  }));
  const rangeLabel = $derived(selection.length > 1 ? `${selection[0]}:${selection.at(-1)}` : activeKey);
  const numbers = $derived(selection.map(key => results.get(key)).filter((value): value is number => typeof value === "number"));
  const total = $derived(numbers.reduce((a, b) => a + b, 0));
  const widthOf = (column: number) => {
    const letter = columnName(column);
    return resizing?.letter === letter ? resizing.width : doc.current.widths[letter] ?? DEFAULT_WIDTH;
  };
  const template = $derived(`40px ${Array.from({ length: COLUMNS }, (_, c) => `${widthOf(c)}px`).join(" ")}`);
  const kind = (value: Value | undefined) =>
    value === undefined || value === "" ? "blank" : isError(value) ? "error" : typeof value === "number" ? "number" : "text";

  // A formula result that changes after an edit gives a small hop so the ripple is visible.
  const shown = new Map<string, string>();
  let bounceTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const changed: string[] = [];
    for (const key of allKeys) {
      if (!inputs[key]?.startsWith("=")) { shown.delete(key); continue; }
      const text = display(results.get(key) ?? "");
      if (shown.has(key) && shown.get(key) !== text) changed.push(key);
      shown.set(key, text);
    }
    if (!changed.length) return;
    bouncing = new Set(changed);
    clearTimeout(bounceTimer);
    bounceTimer = setTimeout(() => bouncing = new Set(), 700);
  });
  onMount(() => {
    const stop = () => { dragging = false; };
    window.addEventListener("pointerup", stop);
    return () => { window.removeEventListener("pointerup", stop); clearTimeout(bounceTimer); };
  });

  function select(point: Point, extend = false) {
    const next = { column: Math.max(0, Math.min(COLUMNS - 1, point.column)), row: Math.max(0, Math.min(ROWS - 1, point.row)) };
    cursor = next;
    if (!extend) anchor = next;
  }
  function write(entries: [string, string][], message: string) {
    doc.change(tx => {
      for (const [key, input] of entries) {
        const cell = doc.current.cells[key];
        if (!cell) { if (input) tx.fields.cells.put(key, { input }); continue; }
        if (!input && cell.tint === undefined && cell.stamp === undefined) tx.fields.cells.delete(key);
        else if (cell.input !== input) tx.fields.cells.entry(key).input.set(input);
      }
    }, { message });
  }
  function decorate(apply: (key: string, handle: ReturnType<typeof doc.fields.cells.entry>) => void, empty: (key: string) => boolean, message: string) {
    doc.change(tx => {
      for (const key of selection) {
        if (!doc.current.cells[key]) {
          if (empty(key)) continue;
          tx.fields.cells.put(key, { input: "" });
        }
        apply(key, tx.fields.cells.entry(key));
        const cell = doc.current.cells[key];
        if (empty(key) && !cell?.input) tx.fields.cells.delete(key);
      }
    }, { message });
  }
  function paint(tint: Tint | null) {
    const clearing = tint === null || selection.every(key => doc.current.cells[key]?.tint === tint);
    const empty = (key: string) => clearing && doc.current.cells[key]?.stamp === undefined;
    decorate((_, cell) => clearing ? cell.tint.clear() : cell.tint.set(tint!), empty, clearing ? "Erase crayon" : "Color cells");
    notice = clearing ? "Crayon erased." : `Colored ${rangeLabel} ${tint}.`;
  }
  function stamp(mark: string | null) {
    const empty = (key: string) => mark === null && doc.current.cells[key]?.tint === undefined;
    decorate((_, cell) => mark === null ? cell.stamp.clear() : cell.stamp.set(mark), empty, mark ? "Stamp cells" : "Remove stamp");
    stampOpen = false;
    notice = mark ? `Stamped ${rangeLabel} ${mark}.` : "Stamp removed.";
    grid.focus();
  }

  async function beginEdit(from: "cell" | "bar", initial?: string) {
    editing = { key: activeKey, from };
    draft = initial ?? doc.current.cells[activeKey]?.input ?? "";
    anchor = cursor;
    if (from === "cell") { await tick(); grid.querySelector<HTMLInputElement>(".pocket-cell-input")?.focus(); }
  }
  function commit(move?: Point) {
    if (!editing) return;
    const { key } = editing;
    editing = null;
    if (draft !== (doc.current.cells[key]?.input ?? "")) write([[key, draft]], "Edit cell");
    if (move) select({ column: cursor.column + move.column, row: cursor.row + move.row });
    grid.focus();
  }
  function cancel() { editing = null; grid.focus(); }
  function editKeys(event: KeyboardEvent) {
    if (event.isComposing) return;
    const moves: Record<string, Point> = { Enter: { column: 0, row: event.shiftKey ? -1 : 1 }, Tab: { column: event.shiftKey ? -1 : 1, row: 0 } };
    // Stop here so the grid does not treat the same key as its own command after commit.
    if (moves[event.key]) { event.preventDefault(); event.stopPropagation(); commit(moves[event.key]); }
    else if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); cancel(); }
  }
  function gridKeys(event: KeyboardEvent) {
    if (editing || event.isComposing) return;
    const arrows: Record<string, Point> = { ArrowUp: { column: 0, row: -1 }, ArrowDown: { column: 0, row: 1 }, ArrowLeft: { column: -1, row: 0 }, ArrowRight: { column: 1, row: 0 } };
    const command = event.metaKey || event.ctrlKey;
    if (arrows[event.key]) {
      event.preventDefault();
      const step = arrows[event.key]!;
      select(command ? { column: step.column ? (step.column < 0 ? 0 : COLUMNS - 1) : cursor.column, row: step.row ? (step.row < 0 ? 0 : ROWS - 1) : cursor.row }
        : { column: cursor.column + step.column, row: cursor.row + step.row }, event.shiftKey);
    } else if (event.key === "Tab") {
      event.preventDefault();
      select({ column: cursor.column + (event.shiftKey ? -1 : 1), row: cursor.row });
    } else if (event.key === "Enter" || event.key === "F2") {
      event.preventDefault();
      void beginEdit("cell");
    } else if (event.key === "Backspace" || event.key === "Delete") {
      event.preventDefault();
      write(selection.map(key => [key, ""]), "Clear cells");
      notice = `Cleared ${rangeLabel}.`;
    } else if (event.key === "Escape") {
      anchor = cursor;
    } else if (command && event.key.toLowerCase() === "a") {
      event.preventDefault();
      anchor = { column: 0, row: 0 }; cursor = { column: COLUMNS - 1, row: ROWS - 1 };
    } else if (command && (event.key.toLowerCase() === "c" || event.key.toLowerCase() === "x")) {
      event.preventDefault();
      void copy(event.key.toLowerCase() === "x");
    } else if (event.key.length === 1 && !command && !event.altKey) {
      event.preventDefault();
      void beginEdit("cell", event.key);
    }
  }

  // Values copy as tab-separated text so they paste into Numbers, Excel, or a note.
  async function copy(cut: boolean) {
    const rows: string[] = [];
    for (let row = bounds.top; row <= bounds.bottom; row++) {
      const line: string[] = [];
      for (let column = bounds.left; column <= bounds.right; column++) line.push(display(results.get(cellKey(column, row)) ?? ""));
      rows.push(line.join("\t"));
    }
    try {
      await navigator.clipboard.writeText(rows.join("\n"));
      if (cut) write(selection.map(key => [key, ""]), "Cut cells");
      notice = `${cut ? "Cut" : "Copied"} ${rangeLabel}.`;
    } catch { notice = "The clipboard is not available here."; }
  }
  function paste(event: ClipboardEvent) {
    if (editing || document.activeElement !== grid) return;
    const text = event.clipboardData?.getData("text/plain");
    if (!text) return;
    event.preventDefault();
    const entries: [string, string][] = [];
    text.replace(/\r/g, "").replace(/\n$/, "").split("\n").forEach((line, r) => line.split("\t").forEach((value, c) => {
      const column = cursor.column + c, row = cursor.row + r;
      if (column < COLUMNS && row < ROWS) entries.push([cellKey(column, row), value.slice(0, 500)]);
    }));
    write(entries, "Paste cells");
    notice = `Pasted ${entries.length} cell${entries.length === 1 ? "" : "s"}.`;
  }

  function resize(node: HTMLElement, column: number) {
    let start = 0, width = 0;
    const letter = columnName(column);
    const move = (event: PointerEvent) => { resizing = { letter, width: Math.max(56, Math.min(320, Math.round(width + event.clientX - start))) }; };
    const up = () => {
      node.removeEventListener("pointermove", move);
      if (resizing) doc.fields.widths.put(letter, resizing.width);
      resizing = null;
    };
    const down = (event: PointerEvent) => {
      event.preventDefault(); event.stopPropagation();
      start = event.clientX; width = widthOf(column);
      node.setPointerCapture(event.pointerId);
      node.addEventListener("pointermove", move);
      node.addEventListener("pointerup", up, { once: true });
    };
    const reset = () => { if (doc.current.widths[letter] !== undefined) doc.fields.widths.delete(letter); };
    node.addEventListener("pointerdown", down);
    node.addEventListener("dblclick", reset);
    return { destroy() { node.removeEventListener("pointerdown", down); node.removeEventListener("dblclick", reset); } };
  }

  // A plain file name; the host still asks where to save it.
  const fileName = (title: string) => (title.replace(/[\\/:*?"<>|\p{Cc}]/gu, " ").replace(/\s+/g, " ").trim().slice(0, 80) || "Pocket Sheet") + ".csv";
  function exportCSV() {
    const link = document.createElement("a");
    link.href = URL.createObjectURL(new Blob([csv(inputs)], { type: "text/csv;charset=utf-8" }));
    link.download = fileName(doc.current.title);
    document.body.append(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(link.href), 60_000);
    notice = "CSV ready. Formulas recalculate when opened in Numbers or Excel.";
  }
</script>

{#snippet sheet(interactive: boolean)}
  <div class="pocket-grid" style:grid-template-columns={template}
    role="grid" aria-readonly={interactive ? undefined : "true"} aria-label={`${doc.current.title || "Pocket Sheet"} cells`} aria-rowcount={ROWS + 1} aria-colcount={COLUMNS + 1}
    aria-multiselectable={interactive ? "true" : undefined} aria-activedescendant={interactive && !editing ? `pocket-${activeKey}` : undefined}
    tabindex={interactive ? 0 : -1} bind:this={() => grid, node => { if (interactive) grid = node; }}
    onkeydown={interactive ? gridKeys : undefined}>
    <div class="pocket-row" role="row">
      <span class="pocket-corner" role="columnheader" aria-label="Rows"></span>
      {#each Array.from({ length: COLUMNS }, (_, c) => c) as column}
        <span class="pocket-colhead" role="columnheader" data-active={interactive && column >= bounds.left && column <= bounds.right ? "" : undefined}>
          {columnName(column)}
          {#if interactive}<span class="pocket-resize" use:resize={column} aria-hidden="true" title="Drag to resize · double-click to reset"></span>{/if}
        </span>
      {/each}
    </div>
    {#each Array.from({ length: ROWS }, (_, r) => r) as row}
      <div class="pocket-row" role="row">
        <span class="pocket-rowhead" role="rowheader" data-active={interactive && row >= bounds.top && row <= bounds.bottom ? "" : undefined}>{row + 1}</span>
        {#each Array.from({ length: COLUMNS }, (_, c) => c) as column}
          {@const key = cellKey(column, row)}
          {@const cell = doc.current.cells[key]}
          {@const value = results.get(key)}
          {@const inside = interactive && column >= bounds.left && column <= bounds.right && row >= bounds.top && row <= bounds.bottom}
          <div class="pocket-cell" id={interactive ? `pocket-${key}` : undefined} role="gridcell" tabindex="-1"
            aria-selected={interactive ? inside : undefined}
            aria-label={`${key}${cell?.stamp ? ` ${cell.stamp}` : ""}: ${display(value ?? "") || "empty"}${cell?.input.startsWith("=") ? `, formula ${cell.input}` : ""}`}
            data-tint={cell?.tint} data-kind={kind(value)} data-formula={cell?.input.startsWith("=") ? "" : undefined}
            data-selected={inside && selection.length > 1 ? "" : undefined} data-active={interactive && key === activeKey ? "" : undefined}
            data-bounce={bouncing.has(key) ? "" : undefined}
            onpointerdown={interactive ? event => { if (event.button !== 0 || editing?.key === key) return; if (editing) commit(); event.preventDefault(); select({ column, row }, event.shiftKey); dragging = true; grid.focus(); } : undefined}
            onpointerenter={interactive ? () => { if (dragging) select({ column, row }, true); } : undefined}
            ondblclick={interactive ? () => { select({ column, row }); void beginEdit("cell"); } : undefined}>
            {#if interactive && editing?.key === key && editing.from === "cell"}
              <input class="pocket-cell-input" aria-label={`Edit ${key}`} bind:value={draft} maxlength="500" spellcheck="false" autocomplete="off"
                onkeydown={editKeys} onblur={() => { if (editing?.key === key) commit(); }} />
            {:else}
              {#if cell?.stamp}<span class="pocket-stamp" aria-hidden="true">{cell.stamp}</span>{/if}
              <span class="pocket-value" title={isError(value ?? "") ? "This formula can't be worked out. Check its references." : undefined}>{display(value ?? "")}</span>
              {#if bouncing.has(key)}<span class="pocket-sparkle" aria-hidden="true">✨</span>{/if}
            {/if}
          </div>
        {/each}
      </div>
    {/each}
  </div>
{/snippet}

<Slop>
  <main class="pocket-desk" onpaste={paste}>
    <section class="pocket-paper">
      <header class="pocket-header">
        <div class="pocket-heading">
          <span class="pocket-eyebrow">Pocket Sheet</span>
          <input class="pocket-title" aria-label="Sheet title" placeholder="Untitled sheet" use:bindText={doc.fields.title} />
        </div>
        <button class="pocket-export" onclick={exportCSV}><Download size={16} strokeWidth={2.2} />Export CSV</button>
      </header>
      <div class="pocket-bar">
        <span class="pocket-address" aria-label="Selected cells">{rangeLabel}</span>
        <span class="pocket-fx" aria-hidden="true">fx</span>
        <input class="pocket-bar-input" aria-label={`Contents of ${activeKey}`} spellcheck="false" autocomplete="off" maxlength="500"
          placeholder="Type a number, words, or =SUM(B2:B5)"
          value={editing ? draft : doc.current.cells[activeKey]?.input ?? ""}
          oninput={event => { if (editing?.from !== "bar") void beginEdit("bar"); draft = event.currentTarget.value; }}
          onfocus={() => { if (!editing) void beginEdit("bar"); }}
          onkeydown={editKeys} onblur={() => { if (editing?.from === "bar") commit(); }} />
      </div>
      <div class="pocket-scroll">{@render sheet(true)}</div>
      <footer class="pocket-tray">
        <div class="pocket-crayons" role="group" aria-label="Crayon">
          {#each tints as tint}
            {@const on = selection.every(key => doc.current.cells[key]?.tint === tint)}
            <button class="pocket-crayon" data-tint={tint} aria-pressed={on} aria-label={`${tint} crayon`} title={`${tint[0]!.toUpperCase()}${tint.slice(1)} crayon`} onclick={() => { paint(tint); grid.focus(); }}><i></i></button>
          {/each}
          <button class="pocket-crayon pocket-eraser" aria-label="Erase crayon" title="Erase crayon" onclick={() => { paint(null); grid.focus(); }}><Eraser size={16} /></button>
        </div>
        <Popover.Root bind:open={stampOpen}>
          <Popover.Trigger class="pocket-stamp-trigger"><span aria-hidden="true">{doc.current.cells[activeKey]?.stamp ?? "⭐"}</span>Stamp</Popover.Trigger>
          <Popover.Portal>
            <Popover.Content class="pocket-stamp-pad" side="top" sideOffset={8} onCloseAutoFocus={event => { event.preventDefault(); grid.focus(); }}>
              <div class="pocket-stamp-grid" role="group" aria-label="Stamps">
                {#each stamps as mark}<button aria-label={`Stamp ${mark}`} onclick={() => stamp(mark)}>{mark}</button>{/each}
              </div>
              <button class="pocket-unstamp" onclick={() => stamp(null)}>No stamp</button>
            </Popover.Content>
          </Popover.Portal>
        </Popover.Root>
        <output class="pocket-sum" aria-live="polite">
          {#if numbers.length}
            <span>Sum <b>{display(total)}</b></span>
            {#if numbers.length > 1}<span>Avg <b>{display(total / numbers.length)}</b></span><span>Count <b>{numbers.length}</b></span>{/if}
          {:else}<span>Select numbers to add them up</span>{/if}
        </output>
      </footer>
    </section>
    <p class="pocket-sr-only" aria-live="polite">{notice}</p>
  </main>

  {#snippet exportView()}
    <article class="pocket-export-view">
      <p class="pocket-eyebrow">Pocket Sheet</p>
      <h1>{doc.current.title || "Untitled sheet"}</h1>
      {@render sheet(false)}
    </article>
  {/snippet}
  {#snippet icon()}
    <div class="pocket-icon" aria-label="Pocket Sheet">
      <div class="pocket-icon-sheet">
        <span></span><b>A</b><b>B</b>
        <b>1</b><i>🍕</i><em>24</em>
        <b>2</b><i></i><em class="pocket-icon-hi">=Σ</em>
      </div>
      <span class="pocket-icon-spark">✨</span>
    </div>
  {/snippet}
</Slop>
