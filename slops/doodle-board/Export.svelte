<script lang="ts">
import doc from "./schema";
import { boardView } from "./drawing";
const view = $derived(boardView(doc.current));
const board = $derived(view.board);
</script>

<div class="doodle-export" style:aspect-ratio={`${board.width} / ${board.height}`}><svg xmlns="http://www.w3.org/2000/svg" viewBox={`0 0 ${board.width} ${board.height}`} role="img" aria-label="Doodle Board artwork"><rect width={board.width} height={board.height} fill="var(--slop-board)" />{#each view.strokes as stroke (stroke.$id)}<path d={stroke.geometry} fill={stroke.color} />{/each}</svg></div>
