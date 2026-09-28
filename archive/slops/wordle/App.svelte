<script lang="ts">
  import { Slop, useDocument } from "@hitslop/document/svelte";
  import { capture } from "@hitslop/document/capture";
  import { Button, Dialog, Progress, Tabs } from "bits-ui";
  import { onMount, tick } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import BarChart3 from "@lucide/svelte/icons/chart-column";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Share2 from "@lucide/svelte/icons/share-2";
  import X from "@lucide/svelte/icons/x";
  import schema, { type Puzzle } from "./schema";
  import {
    evaluateGuess,
    getDailyWord,
    getRandomWord,
    isValidWord,
    type TileState,
  } from "./words";

  const ROWS = 6;
  const COLS = 5;
  const TILE_FLIP_MS = 420;
  const TILE_STAGGER = 80;
  const ROW_FLIP_MS = TILE_FLIP_MS + TILE_STAGGER * (COLS - 1);
  const WIN_PRAISES = ["Genius!", "Magnificent!", "Impressive!", "Splendid!", "Great!", "Phew!"];
  const KEYBOARD_ROWS = [
    ["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"],
    ["A", "S", "D", "F", "G", "H", "J", "K", "L"],
    ["ENTER", "Z", "X", "C", "V", "B", "N", "M", "⌫"],
  ] as const;

  const doc = useDocument(schema);
  const rowReveals = Array.from({ length: ROWS }, () => new Tween(0, { duration: 0, easing: cubicOut }));

  let currentGuess = $state("");
  let shakeRow = $state(false);
  let toastMessage = $state<string | null>(null);
  let announcement = $state("");
  let showStatsModal = $state(false);
  let completionSession = $state(false);
  let revealing = $state(false);
  let pressedKey = $state<string | null>(null);
  let initialized = false;
  let lastGuessCount = 0;
  let lastMode: string | null = null;
  let toastTimer: ReturnType<typeof setTimeout> | null = null;
  let shakeTimer: ReturnType<typeof setTimeout> | null = null;
  let revealTimer: ReturnType<typeof setTimeout> | null = null;

  const activeState = $derived(doc.current.mode === "daily" ? doc.current.daily : doc.current.practice);
  const isGameOver = $derived(activeState.status !== "playing");
  const winPercent = $derived(doc.current.stats.played ? Math.round((doc.current.stats.won / doc.current.stats.played) * 100) : 0);
  const statusLine = $derived(
    activeState.status === "won"
      ? `Solved in ${activeState.guesses.length}/6`
      : activeState.status === "lost"
        ? "Out of guesses"
        : `Guess ${Math.min(ROWS, activeState.guesses.length + 1)} of ${ROWS}`,
  );
  const dialogTitle = $derived(
    completionSession && activeState.status === "won"
      ? (WIN_PRAISES[activeState.guesses.length - 1] ?? "Well done")
      : completionSession && activeState.status === "lost"
        ? "Out of guesses"
        : "Statistics",
  );
  const dialogDescription = $derived(
    completionSession && activeState.status === "won"
      ? `Solved in ${activeState.guesses.length} of ${ROWS}.`
      : completionSession && activeState.status === "lost"
        ? `The word was ${activeState.targetWord}.`
        : `${doc.current.stats.played} games played.`,
  );

  const keyboardLetterStates = $derived.by(() => {
    const letterMap: Record<string, TileState> = {};
    const target = activeState.targetWord;
    for (let row = 0; row < activeState.guesses.length; row += 1) {
      const guess = activeState.guesses[row]!;
      const evaluation = evaluateGuess(guess, target);
      const progress = rowReveals[row]?.current ?? 1;
      for (let col = 0; col < COLS; col += 1) {
        if (tileLocal(progress, col) < 0.5) continue;
        const char = guess[col]!;
        const status = evaluation[col]!;
        const currentBest = letterMap[char];
        if (status === "correct") letterMap[char] = "correct";
        else if (status === "present" && currentBest !== "correct") letterMap[char] = "present";
        else if (!currentBest) letterMap[char] = status;
      }
    }
    return letterMap;
  });

  function tileLocal(progress: number, col: number): number {
    const span = TILE_FLIP_MS / ROW_FLIP_MS;
    const start = (col * TILE_STAGGER) / ROW_FLIP_MS;
    return Math.min(1, Math.max(0, (progress - start) / span));
  }

  function tileRotation(local: number): number {
    if (local <= 0) return 0;
    if (local < 0.5) return local * 180;
    return (1 - local) * 180;
  }

  function tileChar(rowIndex: number, colIndex: number, completedGuess: string | undefined, isCurrentRow: boolean): string {
    if (completedGuess) return completedGuess[colIndex] ?? "";
    if (isCurrentRow) return currentGuess[colIndex] ?? "";
    return "";
  }

  function tileAria(char: string, state: TileState | "", rowIndex: number, colIndex: number): string {
    const slot = `Row ${rowIndex + 1}, column ${colIndex + 1}`;
    if (!char) return `${slot}, empty`;
    if (state === "correct") return `${slot}, ${char}, correct`;
    if (state === "present") return `${slot}, ${char}, in the word`;
    if (state === "absent") return `${slot}, ${char}, not in the word`;
    return `${slot}, ${char}, typed`;
  }

  function keyId(key: string): string {
    if (key === "⌫") return "BACKSPACE";
    return key;
  }

  function keyLabel(key: string): string {
    if (key === "ENTER") return revealing ? "Enter, waiting for tiles" : "Enter";
    if (key === "⌫") return "Backspace";
    const status = keyboardLetterStates[key];
    if (status === "correct") return `${key}, correct`;
    if (status === "present") return `${key}, in the word`;
    if (status === "absent") return `${key}, not in the word`;
    return key;
  }

  function describeEval(guess: string, evaluation: readonly TileState[]): string {
    return evaluation.map((state, index) => {
      const letter = guess[index]!;
      if (state === "correct") return `${letter} correct`;
      if (state === "present") return `${letter} in the word`;
      return `${letter} not in the word`;
    }).join(", ");
  }

  function clearTimer(timer: ReturnType<typeof setTimeout> | null): void {
    if (timer) clearTimeout(timer);
  }

  function showToast(msg: string, duration = 1800): void {
    clearTimer(toastTimer);
    toastMessage = msg;
    toastTimer = setTimeout(() => {
      toastMessage = null;
      toastTimer = null;
    }, duration);
  }

  function announce(msg: string): void {
    announcement = msg;
  }

  function triggerShake(): void {
    if (prefersReducedMotion.current) return;
    shakeRow = true;
    clearTimer(shakeTimer);
    shakeTimer = setTimeout(() => {
      shakeRow = false;
      shakeTimer = null;
    }, 450);
  }

  function afterReveal(callback: () => void): void {
    clearTimer(revealTimer);
    const wait = prefersReducedMotion.current ? 0 : ROW_FLIP_MS + 40;
    revealTimer = setTimeout(() => {
      revealTimer = null;
      callback();
    }, wait);
  }

  function handleKey(raw: string): void {
    if (revealing) return;
    const c = raw.toUpperCase();
    if (c === "ENTER") {
      if (isGameOver) {
        completionSession = false;
        showStatsModal = true;
        return;
      }
      submitGuess();
      return;
    }
    if (isGameOver) return;
    if (c === "BACKSPACE" || c === "DELETE" || c === "⌫") {
      if (currentGuess.length > 0) currentGuess = currentGuess.slice(0, -1);
      return;
    }
    if (/^[A-Z]$/.test(c) && currentGuess.length < COLS) currentGuess += c;
  }

  function submitGuess(): void {
    if (currentGuess.length < COLS) {
      showToast("Not enough letters");
      announce("Not enough letters.");
      triggerShake();
      return;
    }
    if (!isValidWord(currentGuess)) {
      showToast("Not in word list");
      announce("Not in word list.");
      triggerShake();
      return;
    }

    const guess = currentGuess;
    const mode = doc.current.mode;
    const game = mode === "daily" ? doc.current.daily : doc.current.practice;
    const target = game.targetWord;
    const evaluation = evaluateGuess(guess, target);
    const nextCount = game.guesses.length + 1;
    const won = guess === target;
    const lost = !won && nextCount >= ROWS;
    currentGuess = "";
    doc.change((tx) => {
      const board = mode === "daily" ? tx.fields.daily : tx.fields.practice;
      board.guesses.insert(guess);
      if (!won && !lost) return;
      board.status.set(won ? "won" : "lost");
      const stats = doc.current.stats;
      tx.fields.stats.played.set(stats.played + 1);
      if (won) {
        tx.fields.stats.won.set(stats.won + 1);
        const streak = stats.currentStreak + 1;
        tx.fields.stats.currentStreak.set(streak);
        if (streak > stats.maxStreak) tx.fields.stats.maxStreak.set(streak);
        const key = String(nextCount);
        tx.fields.stats.guessDistribution.put(key, (stats.guessDistribution[key] ?? 0) + 1);
      } else {
        tx.fields.stats.currentStreak.set(0);
      }
    });

    revealing = true;
    afterReveal(() => {
      revealing = false;
      const summary = describeEval(guess, evaluation);
      if (won) {
        const praise = WIN_PRAISES[nextCount - 1] ?? "Well done!";
        showToast(praise, 2400);
        announce(`${summary}. ${praise} Solved in ${nextCount} guesses.`);
        completionSession = true;
        showStatsModal = true;
      } else if (lost) {
        showToast(`The word was ${target}`, 3200);
        announce(`${summary}. Out of guesses. The word was ${target}.`);
        completionSession = true;
        showStatsModal = true;
      } else {
        announce(`${summary}. ${ROWS - nextCount} guesses left.`);
      }
    });
  }

  function resetPuzzle(which: "daily" | "practice", next: { targetWord: string; date?: string }): void {
    const count = doc.current[which].guesses.length;
    doc.change((tx) => {
      const board = tx.fields[which];
      board.targetWord.set(next.targetWord);
      if (count) board.guesses.remove(0, count);
      board.status.set("playing");
      if (next.date !== undefined) board.date.set(next.date);
      else if (doc.current[which].date !== undefined) board.date.clear();
    });
  }

  function startNewPractice(): void {
    resetPuzzle("practice", { targetWord: getRandomWord() });
    currentGuess = "";
    completionSession = false;
    revealing = false;
    showStatsModal = false;
    showToast("New practice game");
    announce("New practice game.");
  }

  function setMode(mode: "daily" | "practice"): void {
    if (mode !== doc.current.mode) doc.fields.mode.set(mode);
    currentGuess = "";
    completionSession = false;
    revealing = false;
    announce(mode === "daily" ? "Daily puzzle." : "Practice mode.");
  }

  function shareResult(): void {
    const game = doc.current.mode === "daily" ? doc.current.daily : doc.current.practice;
    const isDaily = doc.current.mode === "daily";
    const score = game.status === "won" ? game.guesses.length : "X";
    const header = isDaily
      ? `hitSlop Wordle (${doc.current.daily.date ?? getDailyWord().dateStr}) ${score}/6`
      : `hitSlop Wordle (Practice) ${score}/6`;
    const emojiRows = game.guesses.map((guess) =>
      evaluateGuess(guess, game.targetWord)
        .map((state) => (state === "correct" ? "🟩" : state === "present" ? "🟨" : "⬛"))
        .join(""),
    );
    const shareText = `${header}\n\n${emojiRows.join("\n")}`;
    if (navigator.clipboard) {
      void navigator.clipboard.writeText(shareText);
      showToast("Copied to clipboard!");
      announce("Result copied to clipboard.");
    }
  }

  function onWindowKeyDown(event: KeyboardEvent): void {
    if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) return;
    if (showStatsModal) return;
    const target = event.target;
    const interactive = target instanceof HTMLElement && target.closest("button, [data-button-root], [data-tabs-trigger], [data-dialog-content], input, textarea");
    if (interactive && (event.key === "Enter" || event.key === " ")) return;
    if (event.repeat && event.key !== "Backspace") return;
    if (!/^[a-zA-Z]$/.test(event.key) && event.key !== "Enter" && event.key !== "Backspace") return;
    event.preventDefault();
    handleKey(event.key);
    pressedKey = event.key === "Backspace" ? "BACKSPACE" : event.key === "Enter" ? "ENTER" : event.key.toUpperCase();
  }

  function onWindowKeyUp(event: KeyboardEvent): void {
    const id = event.key === "Backspace" ? "BACKSPACE" : event.key === "Enter" ? "ENTER" : event.key.toUpperCase();
    if (pressedKey === id) pressedKey = null;
  }

  function snapRows(count: number, duration: number): void {
    for (let i = 0; i < ROWS; i += 1) {
      void rowReveals[i]!.set(i < count ? 1 : 0, { duration, delay: 0 });
    }
  }

  function exportChar(game: Puzzle, rowIndex: number, colIndex: number): string {
    const completed = game.guesses[rowIndex];
    if (completed) return completed[colIndex] ?? "";
    if (game.status === "playing" && rowIndex === game.guesses.length) return currentGuess[colIndex] ?? "";
    return "";
  }

  function exportState(game: Puzzle, rowIndex: number, colIndex: number): TileState | "" {
    const completed = game.guesses[rowIndex];
    if (!completed) return "";
    return evaluateGuess(completed, game.targetWord)[colIndex] ?? "";
  }

  $effect(() => {
    const today = getDailyWord();
    if (doc.current.daily.date !== today.dateStr) {
      resetPuzzle("daily", { targetWord: today.word, date: today.dateStr });
    }
  });

  $effect(() => {
    const mode = doc.current.mode;
    const count = activeState.guesses.length;
    const reduced = prefersReducedMotion.current;
    const modeChanged = lastMode !== null && lastMode !== mode;
    const reset = count < lastGuessCount;
    const instant = !initialized || reduced || modeChanged || reset;
    for (let i = 0; i < ROWS; i += 1) {
      const target = i < count ? 1 : 0;
      const isNewRow = !instant && i === count - 1 && count === lastGuessCount + 1;
      void rowReveals[i]!.set(target, { duration: isNewRow ? ROW_FLIP_MS : 0, delay: 0 });
    }
    lastGuessCount = count;
    lastMode = mode;
    initialized = true;
  });

  onMount(() => {
    window.addEventListener("keydown", onWindowKeyDown);
    window.addEventListener("keyup", onWindowKeyUp);
    const unregister = capture.onPrepare(async () => {
      snapRows(activeState.guesses.length, 0);
      shakeRow = false;
      revealing = false;
      await tick();
    });
    return () => {
      window.removeEventListener("keydown", onWindowKeyDown);
      window.removeEventListener("keyup", onWindowKeyUp);
      unregister();
      clearTimer(toastTimer);
      clearTimer(shakeTimer);
      clearTimer(revealTimer);
      snapRows(activeState.guesses.length, 0);
    };
  });
</script>

<Slop>
  <main class="device" data-slop-selection="none" aria-label="Wordle puzzle chassis">
    <header class="topNav">
      <div class="navGroup">
        <div class="brandTitle">
          <span class="brandDot"></span>
          <span>WORDLE</span>
        </div>
      </div>
      <div class="navGroup" data-slop-export="hide">
        <Tabs.Root
          value={doc.current.mode}
          onValueChange={(value) => { if (value === "daily" || value === "practice") setMode(value); }}
        >
          <Tabs.List class="modeList" aria-label="Game mode">
            <Tabs.Trigger value="daily">Daily</Tabs.Trigger>
            <Tabs.Trigger value="practice">Practice</Tabs.Trigger>
          </Tabs.List>
        </Tabs.Root>
        {#if doc.current.mode === "practice"}
          <Button.Root type="button" class="iconBtn" aria-label="New practice game" onclick={startNewPractice}>
            <RotateCcw size={15} />
          </Button.Root>
        {/if}
        <Button.Root
          type="button"
          class="iconBtn"
          aria-label="Statistics"
          onclick={() => { completionSession = false; showStatsModal = true; }}
        >
          <BarChart3 size={15} />
        </Button.Root>
      </div>
    </header>

    {#if toastMessage}
      <div class="toastWrap" aria-hidden="true">
        <div class="toast">{toastMessage}</div>
      </div>
    {/if}
    <div class="srOnly" aria-live="polite" aria-atomic="true">{announcement}</div>

    <div class="boardWrap">
      <div class="statusLine" aria-hidden="true">{statusLine}</div>
      <div class="grid" role="grid" aria-label="Wordle 6 by 5 game board">
        {#each Array(ROWS) as _, rowIndex}
          {@const completedGuess = activeState.guesses[rowIndex]}
          {@const isCurrentRow = !isGameOver && rowIndex === activeState.guesses.length}
          {@const evaluation = completedGuess ? evaluateGuess(completedGuess, activeState.targetWord) : null}
          {@const progress = rowReveals[rowIndex]!.current}
          <div class="row" class:rowShake={isCurrentRow && shakeRow} role="row">
            {#each Array(COLS) as _, colIndex}
              {@const char = tileChar(rowIndex, colIndex, completedGuess, isCurrentRow)}
              {@const local = completedGuess ? tileLocal(progress, colIndex) : 0}
              {@const revealed = Boolean(completedGuess) && local >= 0.5}
              {@const state = revealed && evaluation ? evaluation[colIndex] ?? "" : ""}
              <div
                class="tile"
                data-filled={Boolean(char)}
                data-pop={Boolean(char) && isCurrentRow && !prefersReducedMotion.current}
                data-state={state}
                style:transform="rotateX({tileRotation(local)}deg)"
                role="gridcell"
                aria-label={tileAria(char, state, rowIndex, colIndex)}
              >
                {char}
              </div>
            {/each}
          </div>
        {/each}
      </div>
    </div>

    <footer class="keyboard" aria-label="On-screen keyboard" data-slop-export="hide">
      {#each KEYBOARD_ROWS as keys}
        <div class="keyboardRow">
          {#each keys as key}
            {@const id = keyId(key)}
            {@const status = id.length === 1 ? (keyboardLetterStates[id] ?? "") : ""}
            <button
              type="button"
              class="key"
              data-wide={key === "ENTER" || key === "⌫"}
              data-state={status}
              data-pressed={pressedKey === id}
              aria-label={keyLabel(key)}
              aria-keyshortcuts={id === "BACKSPACE" ? "Backspace" : id === "ENTER" ? "Enter" : id}
              onpointerdown={() => { pressedKey = id; }}
              onpointerup={() => { if (pressedKey === id) pressedKey = null; }}
              onpointerleave={() => { if (pressedKey === id) pressedKey = null; }}
              onpointercancel={() => { if (pressedKey === id) pressedKey = null; }}
              onclick={() => handleKey(id)}
            >
              {key}
            </button>
          {/each}
        </div>
      {/each}
    </footer>
  </main>

  <Dialog.Root bind:open={showStatsModal}>
    <Dialog.Portal>
      <Dialog.Overlay class="dialogOverlay" data-slop-export="hide" />
      <Dialog.Content class="dialogCard" aria-labelledby="stats-title" aria-describedby="stats-copy" data-slop-export="hide">
        <div class="dialogHead">
          <Dialog.Title id="stats-title"><h2>{dialogTitle}</h2></Dialog.Title>
          <Dialog.Close class="dialogClose" aria-label="Close statistics"><X size={16} /></Dialog.Close>
        </div>
        <Dialog.Description id="stats-copy" class="dialogCopy">{dialogDescription}</Dialog.Description>

        <div class="statsGrid">
          <div class="statItem">
            <span class="statValue">{doc.current.stats.played}</span>
            <span class="statLabel">Played</span>
          </div>
          <div class="statItem">
            <span class="statValue">{winPercent}%</span>
            <span class="statLabel">Win %</span>
          </div>
          <div class="statItem">
            <span class="statValue">{doc.current.stats.currentStreak}</span>
            <span class="statLabel">Current Streak</span>
          </div>
          <div class="statItem">
            <span class="statValue">{doc.current.stats.maxStreak}</span>
            <span class="statLabel">Max Streak</span>
          </div>
        </div>

        <div>
          <div class="distTitle">Guess Distribution</div>
          <div class="distRows">
            {#each [1, 2, 3, 4, 5, 6] as guessNum}
              {@const count = doc.current.stats.guessDistribution[String(guessNum)] || 0}
              {@const maxCount = Math.max(1, ...Object.values(doc.current.stats.guessDistribution))}
              {@const percent = Math.round((count / maxCount) * 100)}
              {@const isCurrentWinGuess = isGameOver && activeState.status === "won" && activeState.guesses.length === guessNum}
              <div class="distRow">
                <span class="distIdx">{guessNum}</span>
                <Progress.Root class="distTrack" value={count} max={maxCount} aria-label="Guessed in {guessNum}: {count}">
                  <div class="distBar" data-highlight={isCurrentWinGuess} style:width="{Math.max(8, percent)}%">{count}</div>
                </Progress.Root>
              </div>
            {/each}
          </div>
        </div>

        <div class="dialogActions">
          {#if isGameOver}
            <Button.Root type="button" class="actionBtn actionPrimary" onclick={shareResult}>
              <Share2 size={14} />
              <span>Share Result</span>
            </Button.Root>
          {/if}
          {#if doc.current.mode === "practice"}
            <Button.Root type="button" class="actionBtn actionSecondary" onclick={startNewPractice}>
              <RotateCcw size={14} />
              <span>New Word</span>
            </Button.Root>
          {/if}
        </div>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>

  {#snippet exportView()}
    {@const game = doc.current.mode === "daily" ? doc.current.daily : doc.current.practice}
    {@const score = game.status === "won" ? `${game.guesses.length}/6` : game.status === "lost" ? "X/6" : `${Math.min(ROWS, game.guesses.length + 1)} of 6`}
    {@const meta = doc.current.mode === "daily" ? `Daily · ${game.date ?? ""} · ${score}` : `Practice · ${score}`}
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
  {/snippet}

  {#snippet icon()}
    {@const badge = activeState.status === "won" ? `${activeState.guesses.length}/6` : activeState.status === "lost" ? "X/6" : "SLOPS"}
    <div class="iconSurface" aria-hidden="true">
      <div class="iconBezel">
        <div class="iconBrandStrip">
          <span class="iconBrandDot"></span>
          <span class="iconBrandTitle">WORDLE</span>
          <span class="iconBrandDot"></span>
        </div>
        <div class="iconGrid">
          <div class="iconRow">
            <div class="iconTile" data-tone="absent">W</div>
            <div class="iconTile" data-tone="present">O</div>
            <div class="iconTile" data-tone="absent">R</div>
            <div class="iconTile" data-tone="absent">D</div>
            <div class="iconTile" data-tone="present">S</div>
          </div>
          <div class="iconRow">
            <div class="iconTile" data-tone="correct">S</div>
            <div class="iconTile" data-tone="correct">L</div>
            <div class="iconTile" data-tone="correct">O</div>
            <div class="iconTile" data-tone="correct">P</div>
            <div class="iconTile" data-tone="correct">S</div>
          </div>
          <div class="iconRow">
            <div class="iconTile" data-tone="empty"></div>
            <div class="iconTile" data-tone="empty"></div>
            <div class="iconTile" data-tone="empty"></div>
            <div class="iconTile" data-tone="empty"></div>
            <div class="iconTile" data-tone="empty"></div>
          </div>
        </div>
        <div class="iconIndicator">
          <div class="iconBar"></div>
          <span class="iconBadge">{badge}</span>
        </div>
      </div>
    </div>
  {/snippet}
</Slop>
