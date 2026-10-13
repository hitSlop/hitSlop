<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { onMount } from "svelte";
  import { Dialog, AlertDialog, DropdownMenu, RadioGroup } from "bits-ui";
  import { isRefused } from "hitslop";
  import { attachments, bindText, getDocument } from "hitslop/svelte";
  import schema, { colors, kinds } from "./schema";
  import {
    addCaregiver, addEntry, babyAge, dateLabel, dayKey, duration, elapsed, entryTitle, journalView, localInput,
    removeEntry, setCaregiverArchived, timeLabel, updateEntry, type Entry, type EntryInput, type Kind,
  } from "./journal";
  import { caregiverInitials, caregiverLabel } from "./shared";
  import Mark from "./Mark.svelte";
  import Choice from "./Choice.svelte";

  const doc = getDocument(schema);
  const labels: Record<Kind, string> = { feeding: "Feeding", diaper: "Diaper", sleep: "Sleep", note: "Note" };
  const prompts: Record<Kind, string> = { feeding: "Log a feed", diaper: "Log a change", sleep: "Log some sleep", note: "Keep a memory" };
  let now = $state(Date.now());
  // The day on screen. View state only: the export cannot see it, so it shows every day.
  let selectedDay = $state(dayKey(Date.now()));
  let profileOpen = $state(false);
  let caregiversOpen = $state(false);
  let entryOpen = $state(false);
  let editingId = $state<string | null>(null);
  let deletingId = $state<string | null>(null);
  let entryError = $state("");
  let birthdayError = $state("");
  let actionError = $state("");
  let announcement = $state("");
  let caregiverName = $state("");
  let caregiverError = $state("");
  let fileInput = $state<HTMLInputElement>();
  let photoError = $state("");
  let importing = $state(false);
  let formKind = $state<Kind>("feeding");
  let formTime = $state("");
  let formEnd = $state("");
  let formCaregiver = $state("");
  let formNotes = $state("");
  let formFeeding = $state("bottle");
  let formAmount = $state("");
  let formUnit = $state("mL");
  let formSide = $state("both");
  let formMinutes = $state("");
  let formDiaper = $state("wet");

  const title = $derived(doc.current.name.trim() ? `${doc.current.name.trim()}’s little days` : "Little Days");
  const initials = $derived(doc.current.name.trim().slice(0, 1).toLocaleUpperCase());
  const today = $derived(dayKey(now));
  const journal = $derived(journalView(doc.current, selectedDay, now));
  const people = $derived(doc.current.caregivers);
  const caregiverOptions = $derived(people.filter(person => !person.archived || (editingId !== null && person.$id === formCaregiver))
    .map(person => ({ value: person.$id, label: `${person.name.trim() || "Unnamed caregiver"}${person.archived ? " (archived)" : ""}` })));
  const photo = $derived(doc.current.photo);
  const photoUrl = $derived(photo ? attachments.url(photo.id) : "");

  function changeDay(offset: number) {
    const date = new Date(`${selectedDay}T12:00:00`);
    date.setDate(date.getDate() + offset);
    selectedDay = dayKey(date);
  }
  function startEntry(kind: Kind, entry?: Entry) {
    now = Date.now();
    formKind = kind;
    editingId = entry?.$id ?? null;
    formTime = localInput(entry?.timestamp ?? Date.now());
    formEnd = localInput(entry?.endTimestamp ?? Date.now());
    formCaregiver = entry?.caregiverId ?? (journal.active.find(person => person.$id === doc.current.lastCaregiverId)?.$id ?? journal.active[0]?.$id ?? "");
    formNotes = entry?.notes ?? "";
    formFeeding = entry?.feedingType ?? "bottle";
    formAmount = entry?.amount?.toString() ?? "";
    formUnit = entry?.unit ?? doc.current.bottleUnit;
    formSide = entry?.side ?? "both";
    formMinutes = entry?.minutes?.toString() ?? "";
    formDiaper = entry?.diaper ?? "wet";
    entryError = "";
    entryOpen = true;
  }
  async function saveEntry(event: SubmitEvent) {
    event.preventDefault();
    entryError = "";
    const clock = Date.now();
    const data = doc.current;
    const id = editingId;
    const input: EntryInput = {
      kind: formKind,
      timestamp: new Date(formTime).getTime(),
      caregiverId: formCaregiver,
      notes: formNotes,
      feedingType: formFeeding === "nursing" ? "nursing" : "bottle",
      unit: formUnit === "oz" ? "oz" : "mL",
      side: formSide === "left" || formSide === "right" ? formSide : "both",
      diaper: formDiaper === "wet" || formDiaper === "dirty" ? formDiaper : "both",
      amount: formAmount === "" ? undefined : Number(formAmount),
      minutes: formMinutes === "" ? undefined : Number(formMinutes),
      endTimestamp: formKind === "sleep" ? new Date(formEnd).getTime() : undefined,
    };
    try {
      if (id) await doc.change(tx => updateEntry(tx, data, id, input, clock));
      else await doc.change(tx => addEntry(tx, data, input, clock));
      selectedDay = dayKey(input.timestamp);
      now = clock;
      entryOpen = false;
      announcement = `${labels[formKind]} ${id ? "updated" : "added"}.`;
    } catch (error) {
      if (!isRefused(error)) throw error;
      entryError = error.message;
    }
  }
  async function deleteEntry() {
    const id = deletingId;
    if (!id) return;
    const data = doc.current;
    try {
      await doc.change(tx => removeEntry(tx, data, id));
      deletingId = null;
      announcement = "Entry deleted.";
    } catch (error) {
      if (!isRefused(error)) throw error;
      actionError = error.message;
    }
  }
  async function submitCaregiver(event: SubmitEvent) {
    event.preventDefault();
    caregiverError = "";
    const data = doc.current;
    try {
      const { id } = await doc.change(tx => addCaregiver(tx, data, caregiverName));
      if (!formCaregiver) formCaregiver = id;
      caregiverName = "";
    } catch (error) {
      if (!isRefused(error)) throw error;
      caregiverError = error.message;
    }
  }
  function setBirthday(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const value = input.value;
    if (value && (!Number.isFinite(new Date(`${value}T00:00:00`).getTime()) || value > dayKey(Date.now()))) {
      birthdayError = "Choose a birthday no later than today.";
      input.value = doc.current.birthday;
      return;
    }
    birthdayError = "";
    doc.fields.birthday.set(value);
  }
  async function importPhoto(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    photoError = "";
    if (!["image/jpeg", "image/png", "image/webp"].includes(file.type)) { photoError = "Choose a JPEG, PNG, or WebP photo."; return; }
    if (file.size > 10 * 1024 * 1024) { photoError = "Choose a photo smaller than 10 MB."; return; }
    importing = true;
    const url = URL.createObjectURL(file);
    try {
      // Check that the browser can open the picture before the host stores it.
      const image = new Image();
      image.src = url;
      await image.decode();
      await attachments.import<typeof schema.descriptor>(file, (tx, ref) => {
        tx.fields.photo.set({ id: ref.id, mimeType: ref.mimeType });
      });
      announcement = "Photo added.";
    } catch (error) {
      photoError = error instanceof Error ? `Couldn’t add this photo. ${error.message}` : "This photo couldn’t be opened. Try another image.";
    } finally { URL.revokeObjectURL(url); importing = false; }
  }
  onMount(() => {
    const timer = setInterval(() => { now = Date.now(); }, 30000);
    return () => clearInterval(timer);
  });
</script>

{#snippet portrait(large = false)}
  <div class="bj-portrait" class:bj-portrait-large={large}>
    {#if photoUrl}<img src={photoUrl} alt={doc.current.name.trim() ? `Photo of ${doc.current.name.trim()}` : "Baby photo"} onerror={() => photoError = "The photo is unavailable. You can choose another in the profile."}/>
    {:else if initials}<span>{initials}</span>
    {:else}<Mark kind="sun" size={large ? 104 : 68}/>{/if}
  </div>
{/snippet}

{#snippet row(entry: Entry, editable: boolean)}
  <article class="bj-entry" data-kind={entry.kind}>
    <time class="bj-entry-time" datetime={Number.isFinite(new Date(entry.timestamp).getTime()) ? new Date(entry.timestamp).toISOString() : undefined}>{timeLabel(entry.timestamp)}</time>
    <span class="bj-entry-mark"><Mark kind={entry.kind} size={34}/></span>
    <div class="bj-entry-copy"><h3>{entryTitle(entry)}</h3>
      {#if entry.kind === "sleep" && entry.endTimestamp !== undefined}<p class="bj-sleep-end">Until {timeLabel(entry.endTimestamp)}{dayKey(entry.endTimestamp) !== dayKey(entry.timestamp) ? ` · ${dateLabel(dayKey(entry.endTimestamp))}` : ""}</p>{/if}
      {#if entry.notes}<p class="bj-entry-note">{entry.notes}</p>{/if}
    </div>
    <div class="bj-by"><span class="bj-person-dot">{caregiverInitials(people, entry.caregiverId)}</span><span>{caregiverLabel(people, entry.caregiverId)}</span></div>
    {#if editable}<DropdownMenu.Root>
      <DropdownMenu.Trigger class="bj-icon-button" aria-label={`Options for ${entryTitle(entry)} at ${timeLabel(entry.timestamp)}`}><Mark kind="more" size={22}/></DropdownMenu.Trigger>
      <DropdownMenu.Portal><DropdownMenu.Content class="bj-options" sideOffset={6}>
        <DropdownMenu.Item class="bj-option" onSelect={() => startEntry(entry.kind, entry)}>Edit entry</DropdownMenu.Item>
        <DropdownMenu.Item class="bj-option bj-danger-text" onSelect={() => { deletingId = entry.$id; actionError = ""; }}>Delete entry…</DropdownMenu.Item>
      </DropdownMenu.Content></DropdownMenu.Portal>
    </DropdownMenu.Root>{/if}
  </article>
{/snippet}

  <main class="bj-app" data-color={doc.current.color}>
    <aside class="bj-sidebar">
      <section class="bj-profile">
        <div class="bj-profile-picture">{@render portrait(true)}<span class="bj-profile-heart"><Mark kind="heart" size={30}/></span></div>
        <h2>{doc.current.name.trim() || "Hello, little one"}<span class="bj-tiny-heart" aria-hidden="true">♥</span></h2>
        <p>{babyAge(doc.current.birthday, now)}</p>
        <button class="bj-pill" onclick={() => profileOpen = true}><Mark kind="edit" size={19}/>{doc.current.name.trim() ? "Edit profile" : "Make it yours"}</button>
      </section>
      <section class="bj-now">
        <h2>Right now <span aria-hidden="true">♡</span></h2>
        {#each [{ kind: "feeding" as const, entry: journal.lastFeed, label: "Last fed" }, { kind: "diaper" as const, entry: journal.lastDiaper, label: "Last diaper" }] as item}
          <button class="bj-recent" onclick={() => startEntry(item.kind, item.entry)}>
            <span class="bj-recent-icon" data-kind={item.kind}><Mark kind={item.kind} size={37}/></span>
            <span class="bj-recent-copy"><span>{item.label}</span><strong>{item.entry ? elapsed(item.entry.timestamp, now) : "Not yet"}</strong><small>{item.entry ? `${timeLabel(item.entry.timestamp)} · ${caregiverLabel(people, item.entry.caregiverId)}` : `Add a ${item.kind === "feeding" ? "feeding" : "change"}`}</small></span>
            <Mark kind="chevron" size={17}/>
          </button>
        {/each}
      </section>
      <section class="bj-memory">
        <div class="bj-section-line"><h2>A little memory</h2><Mark kind="heart" size={24}/></div>
        {#if journal.memory}<button class="bj-memory-content" onclick={() => startEntry("note", journal.memory)}><span>{journal.memory.notes}</span><small>{dateLabel(dayKey(journal.memory.timestamp))}</small></button>
        {:else}<p>The tiny things you’ll want to remember.</p><button class="bj-text-button" onclick={() => startEntry("note")}><Mark kind="plus" size={18}/> Add a little note</button>{/if}
      </section>
      <button class="bj-caregivers-link" onclick={() => caregiversOpen = true}><Mark kind="people" size={21}/> Your circle of care <span>{journal.active.length}</span></button>
    </aside>
    <section class="bj-main">
      <header class="bj-header"><div><p class="bj-eyebrow">A little journal, a lot of love</p><h1>{title}<span aria-hidden="true">♥</span></h1><p class="bj-subtitle">The little things add up.</p></div><div class="bj-sun"><Mark kind="sun" size={82}/></div></header>
      <div class="bj-quick-actions">
        {#each kinds as kind}<button class="bj-quick" data-kind={kind} onclick={() => startEntry(kind)}><Mark {kind} size={43}/><span><strong>{labels[kind]}</strong><small>{prompts[kind]}</small></span></button>{/each}
      </div>
      <section class="bj-journal" aria-label="Daily journal">
        <header class="bj-journal-header"><h2>{selectedDay === today ? "Today, together" : "A day, together"} <span aria-hidden="true">♡</span></h2>
          <div class="bj-date-nav"><button class="bj-icon-button bj-back" aria-label="Previous day" onclick={() => changeDay(-1)}><Mark kind="chevron" size={18}/></button><input type="date" aria-label="Journal date" value={selectedDay} max={today} onchange={event => { if (event.currentTarget.value && event.currentTarget.value <= today) selectedDay = event.currentTarget.value; }}/><button class="bj-icon-button" aria-label="Next day" disabled={selectedDay >= today} onclick={() => changeDay(1)}><Mark kind="chevron" size={18}/></button></div>
        </header>
        <div class="bj-day-summary"><span>{journal.summary.feeds} {journal.summary.feeds === 1 ? "feed" : "feeds"} <i>·</i> {journal.summary.diapers} {journal.summary.diapers === 1 ? "diaper" : "diapers"} <i>·</i> {duration(journal.summary.sleepMinutes)} sleep</span>{#if selectedDay !== today}<button class="bj-text-button" onclick={() => selectedDay = today}>Back to today</button>{/if}</div>
        <div class="bj-timeline">
          {#each journal.dayEntries as entry (entry.$id)}{@render row(entry, true)}{:else}
            <div class="bj-empty"><span class="bj-empty-art"><Mark kind="sleep" size={58}/><span>✦</span><Mark kind="heart" size={26}/></span><h3>{doc.current.entries.length ? "A quiet page" : "Your first little day"}</h3><p>{doc.current.entries.length ? "Nothing recorded on this day yet." : `A feed, a fresh diaper, a sleepy cuddle.
Start with one little moment.`}</p><button class="bj-pill" onclick={() => startEntry("feeding")}><Mark kind="plus" size={18}/> Log a feeding</button></div>
          {/each}
        </div>
        <button class="bj-note-bar" onclick={() => startEntry("note")}><Mark kind="edit" size={22}/><span>Something to remember?</span><span class="bj-note-plus"><Mark kind="plus" size={23}/></span></button>
      </section>
      <footer class="bj-footer"><span>Little moments. Lovingly kept.</span><Mark kind="heart" size={17}/></footer>
    </section>
    <span class="bj-sr-only" aria-live="polite">{announcement}</span>
  </main>

  <Dialog.Root bind:open={entryOpen}>
    <Dialog.Portal><Dialog.Overlay class="bj-overlay"/><Dialog.Content class="bj-dialog" onInteractOutside={event => event.preventDefault()}>
      <div class="bj-dialog-heading"><span class="bj-dialog-mark" data-kind={formKind}><Mark kind={formKind} size={35}/></span><div><Dialog.Title class="bj-dialog-title">{editingId ? "Edit" : "Add"} {labels[formKind].toLowerCase()}</Dialog.Title><Dialog.Description class="bj-dialog-description">{formKind === "note" ? "A small moment worth keeping." : "The details of a little act of care."}</Dialog.Description></div><Dialog.Close class="bj-icon-button" aria-label="Close entry form"><Mark kind="close" size={23}/></Dialog.Close></div>
      <form class="bj-form" onsubmit={saveEntry}>
        {#if formKind === "feeding"}<RadioGroup.Root class="bj-segments" aria-label="Feeding type" bind:value={formFeeding}><RadioGroup.Item value="bottle">Bottle</RadioGroup.Item><RadioGroup.Item value="nursing">Nursing</RadioGroup.Item></RadioGroup.Root>
          {#if formFeeding === "bottle"}<div class="bj-form-pair"><label>Amount <small>(optional)</small><input type="number" step="any" min="0.01" inputmode="decimal" value={formAmount} oninput={event => formAmount = event.currentTarget.value} placeholder="e.g. 120"/></label><div class="bj-field"><span>Unit</span><Choice bind:value={formUnit} label="Bottle unit" options={[{ value: "mL", label: "mL" }, { value: "oz", label: "US fl oz" }]}/></div></div>
          {:else}<div class="bj-form-pair"><div class="bj-field"><span>Side</span><Choice bind:value={formSide} label="Nursing side" options={[{ value: "left", label: "Left" }, { value: "right", label: "Right" }, { value: "both", label: "Both sides" }]}/></div><label>Minutes <small>(optional)</small><input type="number" step="any" min="0.01" inputmode="decimal" value={formMinutes} oninput={event => formMinutes = event.currentTarget.value} placeholder="e.g. 15"/></label></div>{/if}
        {:else if formKind === "diaper"}<RadioGroup.Root class="bj-segments" aria-label="Diaper type" bind:value={formDiaper}><RadioGroup.Item value="wet">Wet</RadioGroup.Item><RadioGroup.Item value="dirty">Dirty</RadioGroup.Item><RadioGroup.Item value="both">Wet + dirty</RadioGroup.Item></RadioGroup.Root>{/if}
        <label>{formKind === "sleep" ? "Sleep started" : "When"}<input type="datetime-local" required bind:value={formTime} max={localInput(now)}/></label>
        {#if formKind === "sleep"}<label>Woke up<input type="datetime-local" required bind:value={formEnd} max={localInput(now)}/></label><p class="bj-help">Use the next day’s date for sleep that crosses midnight.</p>{/if}
        <div class="bj-field"><div class="bj-field-label"><span>Who was there?</span><button type="button" class="bj-text-button" onclick={() => caregiversOpen = true}>Manage caregivers</button></div><Choice bind:value={formCaregiver} options={caregiverOptions} label="Caregiver"/></div>
        <label>{formKind === "note" ? "Your little memory" : "Notes (optional)"}<textarea rows="3" bind:value={formNotes} required={formKind === "note"} placeholder={formKind === "note" ? "A big sleepy smile after breakfast…" : "Anything you’d like to remember…"}></textarea></label>
        {#if entryError}<p class="bj-error" role="alert">{entryError}</p>{/if}
        <div class="bj-form-footer"><Dialog.Close class="bj-button bj-secondary" type="button">Cancel</Dialog.Close><button class="bj-button" type="submit">{editingId ? "Save changes" : `Add ${labels[formKind].toLowerCase()}`}</button></div>
      </form>
    </Dialog.Content></Dialog.Portal>
  </Dialog.Root>

  <Dialog.Root bind:open={profileOpen}>
    <Dialog.Portal><Dialog.Overlay class="bj-overlay"/><Dialog.Content class="bj-dialog">
      <div class="bj-dialog-heading"><div><Dialog.Title class="bj-dialog-title">Your little one</Dialog.Title><Dialog.Description class="bj-dialog-description">Make this little journal theirs.</Dialog.Description></div><Dialog.Close class="bj-icon-button" aria-label="Close profile"><Mark kind="close" size={23}/></Dialog.Close></div>
      <div class="bj-profile-photo-edit">{@render portrait()}<div><input class="bj-sr-only" tabindex="-1" type="file" accept="image/jpeg,image/png,image/webp" bind:this={fileInput} onchange={importPhoto} aria-label="Baby photo"/><button class="bj-pill" disabled={importing} onclick={() => fileInput?.click()}><Mark kind="camera" size={20}/>{importing ? "Adding photo…" : photo ? "Change photo" : "Add a photo"}</button>{#if photo}<button class="bj-text-button" disabled={importing} onclick={() => { doc.fields.photo.clear(); photoError = ""; }}>Remove photo</button>{/if}<p class="bj-help">JPEG, PNG, or WebP · up to 10 MB</p></div></div>
      {#if photoError}<p class="bj-error" role="alert">{photoError}</p>{/if}
      <div class="bj-form"><label>Baby’s name<input placeholder="What’s their name?" use:bindText={doc.fields.name}/></label><label>Birthday <small>(optional)</small><input type="date" value={doc.current.birthday} max={today} onchange={setBirthday}/></label>{#if birthdayError}<p class="bj-error" role="alert">{birthdayError}</p>{/if}<div class="bj-field"><span>A favorite color</span><RadioGroup.Root class="bj-colors" aria-label="Favorite color" value={doc.current.color} onValueChange={value => { if (colors.includes(value as typeof colors[number])) doc.fields.color.set(value as typeof colors[number]); }}>{#each colors as color}<RadioGroup.Item value={color} aria-label={color} style={`--bj-swatch: var(--slop-${color})`}><span>{doc.current.color === color ? "✓" : ""}</span><small>{color}</small></RadioGroup.Item>{/each}</RadioGroup.Root></div><div class="bj-form-footer"><Dialog.Close class="bj-button">Done</Dialog.Close></div></div>
    </Dialog.Content></Dialog.Portal>
  </Dialog.Root>

  <Dialog.Root bind:open={caregiversOpen}>
    <Dialog.Portal><Dialog.Overlay class="bj-overlay"/><Dialog.Content class="bj-dialog">
      <div class="bj-dialog-heading"><div><Dialog.Title class="bj-dialog-title">Your circle of care</Dialog.Title><Dialog.Description class="bj-dialog-description">A name beside every little act of love.</Dialog.Description></div><Dialog.Close class="bj-icon-button" aria-label="Close caregivers"><Mark kind="close" size={23}/></Dialog.Close></div>
      <div class="bj-caregiver-list">{#each people as person (person.$id)}<div class="bj-caregiver-row" class:bj-archived={person.archived}><span class="bj-person-dot">{caregiverInitials(people, person.$id)}</span><input aria-label={`Caregiver name: ${person.name}`} use:bindText={doc.at(person).name}/><button class="bj-text-button" disabled={!person.archived && journal.active.length <= 1} onclick={() => doc.change(tx => setCaregiverArchived(tx, doc.current, person.$id, !person.archived))}>{person.archived ? "Restore" : "Archive"}</button></div>{/each}</div>
      <p class="bj-help">Archived caregivers stay on past entries. Keep at least one caregiver active.</p>
      <form class="bj-caregiver-add" onsubmit={submitCaregiver}><input aria-label="New caregiver name" bind:value={caregiverName} placeholder="Amanda, Grandma, Grandpa…"/><button class="bj-button" disabled={!caregiverName.trim()}>Add</button></form>{#if caregiverError}<p class="bj-error" role="alert">{caregiverError}</p>{/if}<div class="bj-form-footer"><Dialog.Close class="bj-button bj-secondary">Done</Dialog.Close></div>
    </Dialog.Content></Dialog.Portal>
  </Dialog.Root>

  <AlertDialog.Root open={deletingId !== null} onOpenChange={open => { if (!open) deletingId = null; }}><AlertDialog.Portal><AlertDialog.Overlay class="bj-overlay"/><AlertDialog.Content class="bj-dialog bj-confirm"><AlertDialog.Title class="bj-dialog-title">Delete this entry?</AlertDialog.Title><AlertDialog.Description class="bj-dialog-description">This removes the entry from the journal. It can’t be undone here.</AlertDialog.Description>{#if actionError}<p class="bj-error" role="alert">{actionError}</p>{/if}<div class="bj-form-footer"><AlertDialog.Cancel class="bj-button bj-secondary">Keep entry</AlertDialog.Cancel><button class="bj-button bj-destructive" onclick={deleteEntry}>Delete entry</button></div></AlertDialog.Content></AlertDialog.Portal></AlertDialog.Root>
