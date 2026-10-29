<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import { getDocument } from "@hitslop/tinys/svelte";
  import Download from "@lucide/svelte/icons/download";
  import Eraser from "@lucide/svelte/icons/eraser";
  import PaintBucket from "@lucide/svelte/icons/paint-bucket";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Undo2 from "@lucide/svelte/icons/undo-2";
  import { Tabs, ToggleGroup, RadioGroup, Button } from "bits-ui";
  import schema from "./schema";
  import { CELL_COUNT, PALETTES, PALETTE_IDS, SIZE, fillCells, paintCells, setPalette, spriteView } from "./sprite";

  const doc = getDocument(schema);

  type Tool = "pencil" | "eraser" | "fill";
  function isTool(value: string): value is Tool {
    return value === "pencil" || value === "eraser" || value === "fill";
  }

  let tool = $state<Tool>("pencil");
  let painting = $state(false);
  const stroke = new Map<number, string>();
  const { paletteId, palette } = $derived(spriteView(doc.current));

  function previewCell(index: number, color: string): void {
    if ((doc.current.pixels[index] ?? "") === color) return;
    stroke.set(index, color);
    doc.fields.pixels.preview(index, color);
  }
  function commitStroke(): void {
    painting = false;
    if (stroke.size === 0) return;
    const cells = [...stroke.entries()];
    stroke.clear();
    doc.change((tx) => paintCells(tx, doc.current, cells));
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
    if (tool === "fill") {
      doc.change((tx) => fillCells(tx, doc.current, index, doc.current.selectedColor));
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
  // The same undo as Edit > Undo: the person's last change, including a stroke in progress.
  function undo(): void {
    if (stroke.size) commitStroke();
    void doc.undo();
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


<main class="shell" aria-label="Pixel art studio">
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

  <section class="toolRow" data-tiny-export="hide">
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
    <Button.Root type="button" class="tool" onclick={undo} aria-label="Undo">
      <Undo2 size={16} />
    </Button.Root>
    <Button.Root type="button" class="tool exportTool" onclick={exportPng} aria-label="Export PNG">
      <Download size={16} />
    </Button.Root>
  </section>

  <section class="paletteBlock">
    <Tabs.Root value={paletteId} onValueChange={value => { if (value) doc.change(tx => setPalette(tx, doc.current, value)); }} data-tiny-export="hide">
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
