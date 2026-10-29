<script lang="ts">
import "@hitslop/tinys/base.css";
  import "./styles.css";
  import { getDocument } from "@hitslop/tinys/svelte";
  import { Button, Dialog, Progress, Tabs } from "bits-ui";
  import { onMount } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import BarChart3 from "@lucide/svelte/icons/chart-column";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Share2 from "@lucide/svelte/icons/share-2";
  import X from "@lucide/svelte/icons/x";
  import schema from "./schema";
  import { evaluateGuess, dayOf, isValidWord, type TileState } from "./words";
  import { COLS, ROWS, newPractice, playGuess, setMode as writeMode, shareText, wordleView } from "./wordle";

  const doc = getDocument(schema);
  const TILE_FLIP_MS = 420;
  const TILE_STAGGER = 80;
  const ROW_FLIP_MS = TILE_FLIP_MS + TILE_STAGGER * (COLS - 1);
  const WIN_PRAISES = ["Genius!", "Magnificent!", "Impressive!", "Splendid!", "Great!", "Phew!"];
  const KEYBOARD_ROWS = [
    ["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"],
    ["A", "S", "D", "F", "G", "H", "J", "K", "L"],
    ["ENTER", "Z", "X", "C", "V", "B", "N", "M", "⌫"],
  ] as const;

  const rowReveals = Array.from({ length: ROWS }, () => new Tween(0, { duration: 0, easing: cubicOut }));


  // UI-only state: the letters typed so far and the day the board is shown for.
  let currentGuess = $state("");
  let today = $state(dayOf(Date.now()));
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

  const view = $derived(wordleView(doc.current, today));
  const activeState = $derived(view.game);
  const isGameOver = $derived(view.isOver);
  const winPercent = $derived(view.winPercent);
  const statusLine = $derived(view.statusLine);
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
        : `${doc.current.stats.played ?? 0} games played.`,
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

    today = dayOf(Date.now());
    const guess = currentGuess;
    const game = wordleView(doc.current, today).game;
    const target = game.targetWord;
    const evaluation = evaluateGuess(guess, target);
    const nextCount = game.guesses.length + 1;
    const won = guess === target;
    const lost = !won && nextCount >= ROWS;
    currentGuess = "";
    void doc.change((tx) => playGuess(tx, doc.current, today, guess));

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

  function startNewPractice(): void {
    void doc.change((tx) => newPractice(tx, doc.current, Math.random));
    currentGuess = "";
    completionSession = false;
    revealing = false;
    showStatsModal = false;
    showToast("New practice game");
    announce("New practice game.");
  }

  function setMode(mode: "daily" | "practice"): void {
    if (mode !== doc.current.mode) void doc.change((tx) => writeMode(tx, mode));
    currentGuess = "";
    completionSession = false;
    revealing = false;
    announce(mode === "daily" ? "Daily puzzle." : "Practice mode.");
  }

  function shareResult(): void {
    if (navigator.clipboard) {
      void navigator.clipboard.writeText(shareText(doc.current, today));
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
    const refreshDay = () => { today = dayOf(Date.now()); };
    document.addEventListener("visibilitychange", refreshDay);
    return () => {
      window.removeEventListener("keydown", onWindowKeyDown);
      window.removeEventListener("keyup", onWindowKeyUp);
      document.removeEventListener("visibilitychange", refreshDay);
      clearTimer(toastTimer);
      clearTimer(shakeTimer);
      clearTimer(revealTimer);
      snapRows(activeState.guesses.length, 0);
    };
  });
</script>


  <main class="device" aria-label="Wordle puzzle chassis">
    <header class="topNav">
      <div class="navGroup">
        <div class="brandTitle">
          <span class="brandDot"></span>
          <span>WORDLE</span>
        </div>
      </div>
      <div class="navGroup" data-tiny-export="hide">
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

    <footer class="keyboard" aria-label="On-screen keyboard" data-tiny-export="hide">
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
      <Dialog.Overlay class="dialogOverlay" data-tiny-export="hide" />
      <Dialog.Content class="dialogCard" aria-labelledby="stats-title" aria-describedby="stats-copy" data-tiny-export="hide">
        <div class="dialogHead">
          <Dialog.Title id="stats-title"><h2>{dialogTitle}</h2></Dialog.Title>
          <Dialog.Close class="dialogClose" aria-label="Close statistics"><X size={16} /></Dialog.Close>
        </div>
        <Dialog.Description id="stats-copy" class="dialogCopy">{dialogDescription}</Dialog.Description>

        <div class="statsGrid">
          <div class="statItem">
            <span class="statValue">{doc.current.stats.played ?? 0}</span>
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
