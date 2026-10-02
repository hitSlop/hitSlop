<script lang="ts">
import doc from "./schema";
import { boards } from "./drawing";
const board = $derived(boards[doc.current.boardShape]);
</script>

<div class="doodle-export" style:aspect-ratio={`${board.width} / ${board.height}`}><svg xmlns="http://www.w3.org/2000/svg" viewBox={`0 0 ${board.width} ${board.height}`} role="img" aria-label="Doodle Board artwork"><rect width={board.width} height={board.height} fill="var(--slop-board)" />{#each doc.current.strokes as stroke (stroke.$id)}<path d={stroke.geometry} fill={stroke.color} />{/each}</svg></div>
