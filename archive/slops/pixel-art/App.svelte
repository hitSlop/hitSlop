<script lang="ts">
  import { Slop, useDocument } from "@hitslop/document/svelte";
  import Download from "@lucide/svelte/icons/download";
  import Eraser from "@lucide/svelte/icons/eraser";
  import PaintBucket from "@lucide/svelte/icons/paint-bucket";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Undo2 from "@lucide/svelte/icons/undo-2";
  import { Tabs, ToggleGroup, RadioGroup, Button } from "bits-ui";
  import schema from "./schema";

  type Tool = "pencil" | "eraser" | "fill";
  const PALETTE_IDS = ["gameboy", "pico8", "cyberpunk", "mono"] as const;
  type PaletteId = typeof PALETTE_IDS[number];

  const SIZE = 16;
  const CELL_COUNT = SIZE * SIZE;
  const PALETTES: Record<PaletteId, { label: string; colors: string[] }> = {
    gameboy: {
      label: "GB",
      colors: ["#0f380f", "#306230", "#8bac0f", "#9bbc0f"],
    },
    pico8: {
      label: "PICO",
      colors: [
        "#000000", "#1d2b53", "#7e2553", "#008751", "#ab5236", "#5f574f", "#c2c3c7", "#fff1e8",
        "#ff004d", "#ffa300", "#ffec27", "#00e436", "#29adff", "#83769c", "#ff77a8", "#ffccaa",
      ],
    },
    cyberpunk: {
      label: "NEON",
      colors: ["#0b0b12", "#2de2e6", "#ff2bd6", "#f9f002", "#7a3cff", "#ff6b35", "#f8fafc"],
    },
    mono: {
      label: "MONO",
      colors: ["#111111", "#555555", "#999999", "#eeeeee"],
    },
  };
  const FACE = new Set([8, 9, 14, 15, 16, 17, 22, 23, 25, 26, 29, 30, 33, 34, 37, 38, 41, 42, 43, 44, 45, 46, 49, 54]);

  function isPaletteId(value: string): value is PaletteId {
    return (PALETTE_IDS as readonly string[]).includes(value);
  }
  function isTool(value: string): value is Tool {
    return value === "pencil" || value === "eraser" || value === "fill";
  }

  const doc = useDocument(schema);

  let tool = $state<Tool>("pencil");
  let painting = $state(false);
  let undoStack = $state<string[][]>([]);
  const stroke = new Map<number, string>();
  const paletteId = $derived(isPaletteId(doc.current.paletteId) ? doc.current.paletteId : "gameboy");
  const palette = $derived(PALETTES[paletteId]);

  function snapshot(): void {
    undoStack = [...undoStack.slice(-29), [...doc.current.pixels]];
  }
  function previewCell(index: number, color: string): void {
    if ((doc.current.pixels[index] ?? "") === color) return;
    stroke.set(index, color);
    doc.fields.pixels.preview(index, color);
  }
  function commitStroke(): void {
    painting = false;
    if (stroke.size === 0) return;
    const edits = [...stroke.entries()];
    doc.change((tx) => {
      for (const [index, color] of edits) tx.fields.pixels.set(index, color);
    });
    stroke.clear();
  }
  function fill(start: number, color: string): void {
    const pixels = [...doc.current.pixels];
    const target = pixels[start] ?? "";
    if (target === color) return;
    const queue = [start];
    const seen = new Set<number>([start]);
    const edits: number[] = [];
    while (queue.length > 0) {
      const index = queue.pop()!;
      if ((pixels[index] ?? "") !== target) continue;
      pixels[index] = color;
      edits.push(index);
      const x = index % SIZE;
      const y = Math.floor(index / SIZE);
      const neighbors = [
        x > 0 ? index - 1 : -1,
        x < SIZE - 1 ? index + 1 : -1,
        y > 0 ? index - SIZE : -1,
        y < SIZE - 1 ? index + SIZE : -1,
      ];
      for (const next of neighbors) {
        if (next < 0 || seen.has(next)) continue;
        if ((pixels[next] ?? "") === target) {
          seen.add(next);
          queue.push(next);
        }
      }
    }
    doc.change((tx) => {
      for (const index of edits) tx.fields.pixels.set(index, color);
    });
  }
  function pointerIndex(event: PointerEvent, grid: HTMLElement): number | null {
    const rect = grid.getBoundingClientRect();
    const x = Math.floor(((event.clientX - rect.left) / rect.width) * SIZE);
    const y = Math.floor(((event.clientY - rect.top) / rect.height) * SIZE);
    if (x < 0 || y < 0 || x >= SIZE || y >= SIZE) return null;
    return y * SIZE + x;
  }
  function onGridDown(event: PointerEvent): void {
    const grid = event.currentTarget as HTMLElement;
    const index = pointerIndex(event, grid);
    if (index === null) return;
    if (stroke.size) commitStroke();
    grid.setPointerCapture(event.pointerId);
    snapshot();
    if (tool === "fill") {
      fill(index, doc.current.selectedColor);
      return;
    }
    painting = true;
    previewCell(index, tool === "eraser" ? "" : doc.current.selectedColor);
  }
  function onGridMove(event: PointerEvent): void {
    if (!painting) return;
    const grid = event.currentTarget as HTMLElement;
    const index = pointerIndex(event, grid);
    if (index !== null) previewCell(index, tool === "eraser" ? "" : doc.current.selectedColor);
  }
  function onGridUp(): void {
    if (!painting && stroke.size === 0) return;
    commitStroke();
  }
  function undo(): void {
    if (stroke.size) commitStroke();
    const previous = undoStack.at(-1);
    if (!previous) return;
    undoStack = undoStack.slice(0, -1);
    const current = doc.current.pixels;
    doc.change((tx) => {
      for (let index = 0; index < CELL_COUNT; index += 1) {
        const color = previous[index] ?? "";
        if ((current[index] ?? "") !== color) tx.fields.pixels.set(index, color);
      }
    });
  }
  function setPalette(id: string): void {
    if (!isPaletteId(id)) return;
    const colors = PALETTES[id].colors;
    const selected = colors.includes(doc.current.selectedColor) ? doc.current.selectedColor : (colors[0] ?? "#111111");
    doc.change((tx) => {
      if (doc.current.paletteId !== id) tx.fields.paletteId.set(id);
      if (doc.current.selectedColor !== selected) tx.fields.selectedColor.set(selected);
    });
  }
  function exportPng(): void {
    const canvas = document.createElement("canvas");
    const scale = 16;
    canvas.width = SIZE * scale;
    canvas.height = SIZE * scale;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.imageSmoothingEnabled = false;
    for (let i = 0; i < CELL_COUNT; i += 1) {
      const color = doc.current.pixels[i];
      if (!color) continue;
      ctx.fillStyle = color;
      ctx.fillRect((i % SIZE) * scale, Math.floor(i / SIZE) * scale, scale, scale);
    }
    const link = document.createElement("a");
    link.href = canvas.toDataURL("image/png");
    link.download = "sprite.png";
    link.click();
  }
</script>

<Slop>
<main class="shell" data-slop-selection="none" aria-label="Pixel art studio">
  <header class="brand">
    <span class="brandDot"></span>
    <strong>SPRITE POCKET</strong>
    <span class="brandDot"></span>
  </header>

  <section class="screenBezel">
    <div
      class="pixelGrid"
      role="application"
      aria-label="16 by 16 pixel canvas"
      onpointerdown={onGridDown}
      onpointermove={onGridMove}
      onpointerup={onGridUp}
      onpointercancel={onGridUp}
      onpointerleave={onGridUp}
    >
      {#each doc.current.pixels as color, index (index)}
        <span class="cell" style:background={color || "transparent"}></span>
      {/each}
    </div>
  </section>

  <section class="toolRow" data-slop-export="hide">
    <ToggleGroup.Root
      type="single"
      value={tool}
      onValueChange={value => { if (isTool(value)) tool = value; }}
      class="toolGroup"
      aria-label="Drawing tools"
    >
      <ToggleGroup.Item value="pencil" class="tool" aria-label="Pencil"><Pencil size={16} /></ToggleGroup.Item>
      <ToggleGroup.Item value="eraser" class="tool" aria-label="Eraser"><Eraser size={16} /></ToggleGroup.Item>
      <ToggleGroup.Item value="fill" class="tool" aria-label="Fill"><PaintBucket size={16} /></ToggleGroup.Item>
    </ToggleGroup.Root>
    <Button.Root type="button" class="tool" onclick={undo} disabled={undoStack.length === 0} aria-label="Undo">
      <Undo2 size={16} />
    </Button.Root>
    <Button.Root type="button" class="tool exportTool" onclick={exportPng} aria-label="Export PNG">
      <Download size={16} />
    </Button.Root>
  </section>

  <section class="paletteBlock">
    <Tabs.Root value={paletteId} onValueChange={value => { if (value) setPalette(value); }} data-slop-export="hide">
      <Tabs.List class="paletteTabs" aria-label="Palette">
        {#each PALETTE_IDS as id}
          <Tabs.Trigger value={id} class="paletteTab">{PALETTES[id].label}</Tabs.Trigger>
        {/each}
      </Tabs.List>
    </Tabs.Root>
    <RadioGroup.Root class="swatches" value={doc.current.selectedColor} onValueChange={value => { if (value) { doc.fields.selectedColor.set(value); tool = "pencil"; } }} aria-label="Colors">
      {#each palette.colors as color}
        <RadioGroup.Item
          value={color}
          class="swatch"
          style="background:{color}"
          aria-label={color}
        />
      {/each}
    </RadioGroup.Root>
  </section>

  <footer class="hardware" aria-hidden="true">
    <div class="dpad">
      <i class="dpadArm dpadUp"></i>
      <i class="dpadArm dpadLeft"></i>
      <i class="dpadArm dpadRight"></i>
      <i class="dpadArm dpadDown"></i>
    </div>
    <div class="faceButtons">
      <span class="faceButton faceButtonOffset">B</span>
      <span class="faceButton">A</span>
    </div>
  </footer>
</main>

{#snippet exportView()}
  <article class="exportSheet" aria-label="Exported 16 by 16 sprite">
    <div class="exportBezel">
      <div class="exportGrid">
        {#each doc.current.pixels as color, index (index)}
          <span class="cell" style:background={color || "transparent"}></span>
        {/each}
      </div>
    </div>
  </article>
{/snippet}

{#snippet icon()}
  <div class="iconSurface" aria-hidden="true">
    <article class="iconBody">
      <div class="iconBezel">
        <div class="iconScreen">
          {#each Array.from({ length: 64 }, (_, index) => index) as index}
            <i class="iconPixel" data-on={FACE.has(index)}></i>
          {/each}
        </div>
      </div>
      <div class="iconControls">
        <span class="iconDpad"></span>
        <span class="iconBtns"><i class="iconBtn iconBtnOffset"></i><i class="iconBtn"></i></span>
      </div>
    </article>
  </div>
{/snippet}
</Slop>
