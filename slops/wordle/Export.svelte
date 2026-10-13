<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { COLS, ROWS, wordleView } from "./wordle";
  import { dayOf, evaluateGuess, type TileState } from "./words";
  let { snapshot }: CaptureProps<typeof schema> = $props();

  type Game = ReturnType<typeof wordleView>["game"];
  const today = dayOf(Date.now());
  // The typed-but-unsubmitted row is view state and is not exported.
  function exportChar(game: Game, rowIndex: number, colIndex: number): string {
    return game.guesses[rowIndex]?.[colIndex] ?? "";
  }
  function exportState(game: Game, rowIndex: number, colIndex: number): TileState | "" {
    const completed = game.guesses[rowIndex];
    if (!completed) return "";
    return evaluateGuess(completed, game.targetWord)[colIndex] ?? "";
  }
  const view = $derived(wordleView(snapshot, today));
  const game = $derived(view.game);
  const meta = $derived(view.mode === "daily" ? `Daily · ${game.date ?? ""} · ${view.score}` : `Practice · ${view.score}`);
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
      {view.statusLine}
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
