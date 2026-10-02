<script lang="ts">
  import { bindText } from "@hitslop/document/svelte";
  import { Checkbox } from "bits-ui";
  import Check from "@lucide/svelte/icons/check";
  import doc from "./schema";
  import { CELLS, formatDeadline, keyOf, ringIndex, sheetView } from "./chart";

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

  const { doneCount, deadlineText, isDone, textOf } = $derived(sheetView(doc.current));
  const deadlineLabel = $derived(formatDeadline(doc.current.deadline));
  const namedThemes = $derived(doc.current.themes.filter((theme) => theme.title.trim()).length);
  const writtenActions = $derived(doc.current.themes.reduce((count, theme) => count + theme.cells.filter((cell) => cell.trim()).length, 0));
  const guide = $derived(
    !doc.current.goal.trim() ? "Write the one goal in the centre."
      : namedThemes < 8 ? "Name the eight themes that ring it."
        : writtenActions < 8 ? "Give each theme eight concrete actions."
          : "",
  );

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

  function setDone(theme: number, action: number, done: boolean): void {
    const key = keyOf(theme, action);
    if (done) doc.fields.done.put(key, true);
    else doc.fields.done.delete(key);
  }
  function titleOf(theme: number): string {
    return doc.current.themes[theme]?.title.trim() || `Theme ${theme + 1}`;
  }
</script>


  <main class="canvas" bind:this={canvasEl} aria-label="Open Window 64 chart">
    <article class="page">
      <header class="masthead">
        <div class="crest" aria-hidden="true"></div>
        <div>
          <p class="wordmark">Open Window 64</p>
          <h1 class="title">Harada Method</h1>
        </div>
        <label class="target">
          <span>Target</span>
          <input type="date" aria-label="Target date" bind:value={doc.fields.deadline.value} />
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
