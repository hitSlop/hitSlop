<script lang="ts">
  import { AlertDialog, Dialog, Slider, Switch } from "bits-ui";
  import X from "@lucide/svelte/icons/x";
  import { getDocument, notify } from "@hitslop/tinys/svelte";
  import schema from "./schema";
  import { ASSUMED_EXAM_DAYS, cardCap, resetProgress, setBudget, setExamDate, setRetention, type Budget, type DeckView } from "./deck";

  interface Props { open: boolean; today: string; view: DeckView }
  let { open = $bindable(), today, view }: Props = $props();
  const doc = getDocument(schema);
  let confirmReset = $state(false);

  const data = $derived(doc.current);
  const retention = $derived(data.scheduler.requestRetention);
  const automatic = $derived(data.newPerDay === undefined);

  const whole = (input: HTMLInputElement) => (input.value.trim() === "" ? NaN : Number(input.value));

  function changeExam(event: Event & { currentTarget: HTMLInputElement }) {
    const value = event.currentTarget.value;
    void doc.change(tx => setExamDate(tx, value || undefined, today));
  }
  function previewRetention(value: number) {
    doc.fields.scheduler.requestRetention.preview(Math.round(value * 100) / 100);
  }
  function commitRetention(value: number) {
    void doc.change(tx => setRetention(tx, Math.round(value * 100) / 100));
  }
  function changeBudget(key: "dailyMinutes" | "secondsPerCard" | "newPerDay", input: HTMLInputElement) {
    const value = whole(input);
    const budget: Budget = { [key]: value };
    // Afterwards the field shows the saved value; a refusal is reported by the host.
    void doc.change(tx => setBudget(tx, budget)).finally(() => {
      input.value = String(doc.current[key] ?? "");
    });
  }
  function setAutomatic(on: boolean) {
    const suggested = Math.max(1, Math.min(1000, view.forecast?.today.newToday || 10));
    void doc.change(tx => setBudget(tx, { newPerDay: on ? null : suggested }));
  }
  async function reset() {
    const count = await doc.change(tx => resetProgress(tx, doc.current));
    confirmReset = false;
    notify(`${count} cards are new again and the log is clear.`);
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="deck-overlay" />
    <Dialog.Content class="deck-dialog">
      <div class="deck-dialog-head">
        <Dialog.Title class="deck-dialog-title">Settings</Dialog.Title>
        <Dialog.Close class="deck-icon-button" aria-label="Close settings"><X size={20} /></Dialog.Close>
      </div>
      <Dialog.Description class="deck-dialog-description">What the plan aims for and how much time you have.</Dialog.Description>

      <div class="deck-settings">
        <div class="deck-setting">
          <label for="deck-exam">Exam date</label>
          <div class="deck-setting-row">
            <input id="deck-exam" type="date" min={today} value={data.examDate ?? ""} onchange={changeExam} />
            {#if data.examDate}
              <button class="deck-text-button" onclick={() => doc.change(tx => setExamDate(tx, undefined, today))}>Clear</button>
            {/if}
          </div>
          <p class="deck-setting-note">{data.examDate ? "New cards are paced to be learned two days before it." : `Not set: the plan assumes an exam ${ASSUMED_EXAM_DAYS} days from today.`}</p>
        </div>

        <div class="deck-setting">
          <span class="deck-setting-label" id="deck-retention-label">Target retention <output>{Math.round(retention * 100)}%</output></span>
          <Slider.Root type="single" value={retention} min={0.7} max={0.99} step={0.01} onValueChange={previewRetention} onValueCommit={commitRetention} class="deck-slider" aria-labelledby="deck-retention-label">
            {#snippet children({ thumbs })}
              <span class="deck-slider-track"><Slider.Range class="deck-slider-range" /></span>
              {#each thumbs as index (index)}
                <Slider.Thumb {index} class="deck-slider-thumb" aria-label="Target retention" />
              {/each}
            {/snippet}
          </Slider.Root>
          <p class="deck-setting-note">The chance of remembering a card when it comes due. Higher means more reviews.</p>
        </div>

        <div class="deck-setting deck-setting-pair">
          <div>
            <label for="deck-minutes">Minutes a day</label>
            <input id="deck-minutes" type="number" inputmode="numeric" min="1" max="600" step="1" value={data.dailyMinutes} onchange={event => changeBudget("dailyMinutes", event.currentTarget)} />
          </div>
          <div>
            <label for="deck-seconds">Seconds a card</label>
            <input id="deck-seconds" type="number" inputmode="numeric" min="3" max="600" step="1" value={data.secondsPerCard} onchange={event => changeBudget("secondsPerCard", event.currentTarget)} />
          </div>
          <p class="deck-setting-note">Daily cap: {cardCap(data)} cards. Cards past the cap wait for the next day.</p>
        </div>

        <div class="deck-setting">
          <div class="deck-setting-row deck-setting-switch">
            <label for="deck-auto">Pace new cards automatically</label>
            <Switch.Root id="deck-auto" class="deck-switch" checked={automatic} onCheckedChange={setAutomatic}>
              <Switch.Thumb class="deck-switch-thumb" />
            </Switch.Root>
          </div>
          {#if !automatic}
            <div class="deck-setting-row">
              <label for="deck-new">New cards a day</label>
              <input id="deck-new" type="number" inputmode="numeric" min="1" max="1000" step="1" value={data.newPerDay} onchange={event => changeBudget("newPerDay", event.currentTarget)} />
            </div>
          {/if}
          <p class="deck-setting-note">{automatic ? "Unseen cards are spread evenly until two days before the exam." : "A fixed number of unseen cards each day, within the budget."}</p>
        </div>

        <div class="deck-setting deck-setting-danger">
          <button class="deck-text-button deck-danger" onclick={() => (confirmReset = true)} disabled={!data.reviews.length && data.cards.every(card => card.state === "new")}>Reset progress…</button>
          <p class="deck-setting-note">Every card back to new and the review log cleared. Cards stay.</p>
        </div>
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<AlertDialog.Root bind:open={confirmReset}>
  <AlertDialog.Portal>
    <AlertDialog.Overlay class="deck-overlay" />
    <AlertDialog.Content class="deck-dialog deck-confirm">
      <AlertDialog.Title class="deck-dialog-title">Reset all progress?</AlertDialog.Title>
      <AlertDialog.Description class="deck-dialog-description">All {data.cards.length} cards become new and {data.reviews.length} logged reviews are removed. Undo (⌘Z) can bring them back.</AlertDialog.Description>
      <div class="deck-dialog-actions">
        <AlertDialog.Cancel class="deck-text-button">Keep progress</AlertDialog.Cancel>
        <AlertDialog.Action class="deck-primary deck-danger-fill" onclick={reset}>Reset progress</AlertDialog.Action>
      </div>
    </AlertDialog.Content>
  </AlertDialog.Portal>
</AlertDialog.Root>
