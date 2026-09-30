<script lang="ts">
  import { Slop, bindText, bindValue, useDocument } from "@hitslop/document/svelte";
  import { Checkbox } from "bits-ui";
  import Check from "@lucide/svelte/icons/check";
  import schema from "./schema";
  import { CELLS, formatDate, formatDeadline, keyOf, ringIndex } from "./chart";

  type CellBind = {
    value: string;
    preview: (value: string) => void;
    set: (value: string) => void;
  };

  function bindCell(node: HTMLTextAreaElement, binding: CellBind) {
    let current = binding;
    let focused = false;
    let baseline = binding.value;
    const sync = () => {
      if (focused) return;
      if (node.value !== current.value) node.value = current.value;
      baseline = node.value;
    };
    const onFocus = () => {
      focused = true;
      baseline = node.value;
    };
    const onInput = () => {
      focused = true;
      if (node.value !== current.value) current.preview(node.value);
    };
    const commit = () => {
      const dirty = node.value !== baseline;
      focused = false;
      if (dirty) current.set(node.value);
      baseline = node.value;
    };
    node.addEventListener("focus", onFocus);
    node.addEventListener("input", onInput);
    node.addEventListener("change", commit);
    node.addEventListener("blur", commit);
    sync();
    return {
      update(next: CellBind) {
        current = next;
        sync();
      },
      destroy() {
        if (focused) commit();
        node.removeEventListener("focus", onFocus);
        node.removeEventListener("input", onInput);
        node.removeEventListener("change", commit);
        node.removeEventListener("blur", commit);
      },
    };
  }

  const doc = useDocument(schema);
  const doneCount = $derived(Object.values(doc.current.done).filter(Boolean).length);
  const deadlineLabel = $derived(formatDeadline(doc.current.deadline));
  const namedThemes = $derived(doc.current.themes.filter((theme) => theme.title.trim()).length);
  const writtenActions = $derived(doc.current.themes.reduce((count, theme) => count + theme.cells.filter((cell) => cell.trim()).length, 0));
  const guide = $derived(
    !doc.current.goal.trim() ? "Write the one goal in the centre."
      : namedThemes < 8 ? "Name the eight themes that ring it."
        : writtenActions < 8 ? "Give each theme eight concrete actions."
          : "",
  );
  const deadlineText = $derived(formatDate(doc.current.deadline));

  let canvasEl = $state<HTMLElement | null>(null);
  let canvasWidth = $state(9999);
  let focused = $state(4);
  const zoomed = $derived(canvasWidth < 560);
  const blockLabel = $derived(focused === 4 ? "The goal and its eight themes" : titleOf(ringIndex(Math.floor(focused / 3), focused % 3)));

  $effect(() => {
    const element = canvasEl;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => { canvasWidth = entry?.contentRect.width ?? 9999; });
    observer.observe(element);
    return () => observer.disconnect();
  });

  function isDone(theme: number, action: number): boolean {
    return doc.current.done[keyOf(theme, action)] === true;
  }
  function setDone(theme: number, action: number, done: boolean): void {
    const key = keyOf(theme, action);
    if (done) doc.fields.done.put(key, true);
    else doc.fields.done.delete(key);
  }
  function titleOf(theme: number): string {
    return doc.current.themes[theme]?.title.trim() || `Theme ${theme + 1}`;
  }
  function textOf(cell: (typeof CELLS)[number]): string {
    if (cell.role === "goal") return doc.current.goal;
    const theme = doc.current.themes[cell.theme];
    if (!theme) return "";
    if (cell.role === "theme") return theme.title;
    return theme.cells[cell.action] ?? "";
  }
</script>

<Slop>
  <main class="canvas" data-slop-selection="none" bind:this={canvasEl} aria-label="Open Window 64 chart">
    <article class="page">
      <header class="masthead">
        <div class="crest" aria-hidden="true"></div>
        <div>
          <p class="wordmark">Open Window 64</p>
          <h1 class="title">Harada Method</h1>
        </div>
        <label class="target">
          <span>Target</span>
          <input type="date" aria-label="Target date" use:bindValue={doc.fields.deadline} />
          <em class="targetNote">{deadlineLabel}</em>
        </label>
        <div class="tally">
          <p class="tallyCount"><b>{doneCount}</b><small>/ 64</small></p>
          <div class="tallyBar" aria-hidden="true"><span class="tallyFill" style:width={`${(doneCount / 64) * 100}%`}></span></div>
          <p class="tallyLabel">Actions taken</p>
        </div>
      </header>

      {#if guide}
        <p class="guide" data-slop-export="hide">{guide}</p>
      {/if}

      {#if zoomed}
        <nav class="zoomBar" data-slop-export="hide" aria-label="Choose a block">
          {#each { length: 9 } as _, block (block)}
            <button type="button" class="zoomKey" data-on={focused === block} aria-pressed={focused === block} onclick={() => { focused = block; }}>
              {block === 4 ? "Goal" : titleOf(ringIndex(Math.floor(block / 3), block % 3))}
            </button>
          {/each}
        </nav>
        <p class="zoomTitle" data-slop-export="hide">{blockLabel}</p>
      {/if}

      <div class="sheet" data-zoom={zoomed ? "on" : "off"}>
        {#each CELLS as cell (cell.row * 9 + cell.col)}
          {@const theme = cell.role === "goal" ? undefined : doc.current.themes[cell.theme]}
          <div
            class="cell"
            class:cellAway={zoomed && cell.block !== focused}
            data-role={cell.role}
            data-x={cell.edgeX}
            data-y={cell.edgeY}
            data-done={cell.role === "action" && isDone(cell.theme, cell.action)}
          >
            {#if cell.role === "goal"}
              <textarea class="write" aria-label="Core goal" placeholder="The one goal" use:bindText={doc.fields.goal}></textarea>
            {:else if cell.role === "theme" && theme}
              <textarea class="write" aria-label="Theme {cell.theme + 1}" placeholder="Theme {cell.theme + 1}" use:bindText={doc.at(theme).title}></textarea>
            {:else if theme && cell.action < theme.cells.length}
              <textarea
                class="write"
                aria-label="{titleOf(cell.theme)}, action {cell.action + 1}"
                placeholder="—"
                use:bindCell={{
                  value: theme.cells[cell.action] ?? "",
                  preview: (value) => doc.at(theme).cells.preview(cell.action, value),
                  set: (value) => doc.at(theme).cells.set(cell.action, value),
                }}
              ></textarea>
              <Checkbox.Root
                class="tick"
                checked={isDone(cell.theme, cell.action)}
                onCheckedChange={(checked) => setDone(cell.theme, cell.action, checked === true)}
                aria-label="Mark done: {titleOf(cell.theme)}, action {cell.action + 1}"
              >
                {#snippet children({ checked })}{#if checked}<Check size={8} strokeWidth={3} />{/if}{/snippet}
              </Checkbox.Root>
            {/if}
          </div>
        {/each}
      </div>
    </article>
  </main>

  {#snippet exportView()}
    <article class="exportPage" aria-label="Exported Open Window 64 chart">
      <header class="masthead">
        <div class="crest" aria-hidden="true"></div>
        <div>
          <p class="wordmark">Open Window 64</p>
          <h1 class="title">Harada Method</h1>
        </div>
        <div class="target">
          <span>Target</span>
          <strong>{deadlineText || "No date set"}</strong>
        </div>
        <div class="tally">
          <p class="tallyCount"><b>{doneCount}</b><small>/ 64</small></p>
          <div class="tallyBar" aria-hidden="true"><span class="tallyFill" style:width={`${(doneCount / 64) * 100}%`}></span></div>
          <p class="tallyLabel">Actions taken</p>
        </div>
      </header>

      <div class="exportSheet">
        {#each CELLS as cell (cell.row * 9 + cell.col)}
          <div
            class="cell"
            data-role={cell.role}
            data-x={cell.edgeX}
            data-y={cell.edgeY}
            data-done={cell.role === "action" && isDone(cell.theme, cell.action)}
          >
            <div class="write">{textOf(cell)}</div>
            {#if cell.role === "action" && isDone(cell.theme, cell.action)}
              <span class="tick" data-checkbox-root data-state="checked" aria-hidden="true"><Check size={8} strokeWidth={3} /></span>
            {/if}
          </div>
        {/each}
      </div>
    </article>
  {/snippet}

  {#snippet icon()}
    <div class="iconSurface" aria-hidden="true">
      <article class="iconPage">
        <div class="iconGrid">
          {#each { length: 9 } as _, index (index)}
            <span class="iconCell" data-role={index === 4 ? "goal" : "theme"}></span>
          {/each}
        </div>
      </article>
    </div>
  {/snippet}
</Slop>
