<script lang="ts">
import "@hitslop/tinys/base.css";
import "./styles.css";
import type { CaptureProps } from "@hitslop/tinys/svelte";
import type schema from "./schema";
import { boardView } from "./drawing";
let { snapshot }: CaptureProps<typeof schema> = $props();
const view = $derived(boardView(snapshot));
const board = $derived(view.board);
</script>

<div class="doodle-export" style:aspect-ratio={`${board.width} / ${board.height}`}><svg xmlns="http://www.w3.org/2000/svg" viewBox={`0 0 ${board.width} ${board.height}`} role="img" aria-label="Doodle Board artwork"><rect width={board.width} height={board.height} fill="var(--tiny-board)" />{#each view.strokes as stroke (stroke.$id)}<path d={stroke.geometry} fill={stroke.color} />{/each}</svg></div>
