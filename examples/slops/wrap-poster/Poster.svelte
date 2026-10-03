<script lang="ts">
  import { prepareWithSegments } from "@chenglou/pretext";
  import { flow, type Obstacle } from "./flow";
  import { BODY, PAGE, TITLE, fonts } from "./fonts.svelte";
  import type { PosterShape } from "./types";

  interface Props {
    title: string;
    body: string;
    shapes: PosterShape[];
    selected?: string | null;
    interactive?: boolean;
    fits?: boolean;
    onshapedown?: (event: PointerEvent, id: string) => void;
    onshapekey?: (event: KeyboardEvent, id: string) => void;
  }
  let { title, body, shapes, selected = null, interactive = false, fits = $bindable(true), onshapedown, onshapekey }: Props = $props();

  // Preparing (measuring) text is the costly step and only depends on the words; laying it out is cheap and runs on every drag.
  const preparedTitle = $derived(fonts.ready ? prepareWithSegments(title.trim() || " ", TITLE.font, { whiteSpace: "pre-wrap" }) : null);
  const preparedBody = $derived(fonts.ready ? prepareWithSegments(body, BODY.font, { whiteSpace: "pre-wrap" }) : null);

  const obstacles = $derived<Obstacle[]>(shapes.map((shape) => {
    const size = shape.size * PAGE.width;
    const cx = shape.x * PAGE.width;
    const cy = shape.y * PAGE.height;
    return shape.kind === "circle" ? { kind: "circle", cx, cy, r: size / 2 } : { kind: "square", x: cx - size / 2, y: cy - size / 2, size };
  }));

  const laid = $derived.by(() => {
    if (!preparedTitle || !preparedBody) return { title: [], body: [], fits: true };
    const frame = { left: PAGE.side, right: PAGE.width - PAGE.side, gap: 12, minSpan: 64 };
    const head = flow(preparedTitle, { ...frame, top: PAGE.top, bottom: PAGE.height * 0.42, lineHeight: TITLE.lineHeight }, obstacles);
    const main = flow(preparedBody, { ...frame, top: head.bottom + 14, bottom: PAGE.height - PAGE.bottom, lineHeight: BODY.lineHeight }, obstacles);
    return { title: head.lines, body: main.lines, fits: head.fits && main.fits };
  });
  $effect(() => { fits = laid.fits; });
</script>

<div class="page" style:width="{PAGE.width}px" style:height="{PAGE.height}px">
  <h1 class="sr-only">{title}</h1>
  <p class="sr-only">{body}</p>

  {#each shapes as shape (shape.id)}
    {@const id = shape.id}
    {@const size = shape.size * PAGE.width}
    <svelte:element
      this={interactive ? "button" : "div"}
      class="shape"
      type={interactive ? "button" : undefined}
      data-kind={shape.kind}
      data-tone={shape.tone}
      data-selected={interactive && selected === id}
      aria-label={interactive ? `${shape.tone} ${shape.kind}. Drag or use arrow keys to move, plus and minus to resize.` : undefined}
      aria-hidden={interactive ? undefined : true}
      style:left="{shape.x * PAGE.width - size / 2}px"
      style:top="{shape.y * PAGE.height - size / 2}px"
      style:width="{size}px"
      style:height="{size}px"
      onpointerdown={interactive ? (event: PointerEvent) => onshapedown?.(event, id) : undefined}
      onkeydown={interactive ? (event: KeyboardEvent) => onshapekey?.(event, id) : undefined}
    ></svelte:element>
  {/each}

  {#each laid.title as line, i (i)}
    <span class="line head" aria-hidden="true" style:left="{line.x}px" style:top="{line.y}px">{line.text}</span>
  {/each}
  {#each laid.body as line, i (i)}
    <span class="line" aria-hidden="true" style:left="{line.x}px" style:top="{line.y}px">{line.text}</span>
  {/each}
</div>
