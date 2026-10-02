<script lang="ts">
import { ui } from "./ui.svelte";
import doc, { type Puzzle } from "./schema";
import { evaluateGuess, type TileState } from "./words";
const ROWS = 6;
const COLS = 5;
function exportChar(game: Puzzle, rowIndex: number, colIndex: number): string {
  const completed = game.guesses[rowIndex];
  if (completed) return completed[colIndex] ?? "";
  if (game.status === "playing" && rowIndex === game.guesses.length) return ui.currentGuess[colIndex] ?? "";
  return "";
}
function exportState(game: Puzzle, rowIndex: number, colIndex: number): TileState | "" {
  const completed = game.guesses[rowIndex];
  if (!completed) return "";
  return evaluateGuess(completed, game.targetWord)[colIndex] ?? "";
}
const game = $derived(doc.current.mode === "daily" ? doc.current.daily : doc.current.practice);
const score = $derived(game.status === "won" ? `${game.guesses.length}/6` : game.status === "lost" ? "X/6" : `${Math.min(ROWS, game.guesses.length + 1)} of 6`);
const meta = $derived(doc.current.mode === "daily" ? `Daily · ${game.date ?? ""} · ${score}` : `Practice · ${score}`);
</script>

<article class="exportDevice" aria-label="Exported Wordle puzzle">
  <header class="exportHead">
    <div class="brandTitle">
      <span class="brandDot"></span>
      <span>WORDLE</span>
    </div>
    <span class="exportMeta">{meta}</span>
  </header>
  <div class="exportBoard">
    <div class="statusLine">
      {game.status === "won" ? `Solved in ${game.guesses.length}/6` : game.status === "lost" ? "Out of guesses" : `Guess ${Math.min(ROWS, game.guesses.length + 1)} of ${ROWS}`}
    </div>
    <div class="grid" role="grid" aria-label="Wordle board">
      {#each Array(ROWS) as _, rowIndex}
        <div class="row" role="row">
          {#each Array(COLS) as _, colIndex}
            {@const char = exportChar(game, rowIndex, colIndex)}
            {@const state = exportState(game, rowIndex, colIndex)}
            <div class="tile" data-filled={Boolean(char)} data-state={state} role="gridcell">{char}</div>
          {/each}
        </div>
      {/each}
    </div>
  </div>
</article>
