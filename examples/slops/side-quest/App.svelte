<script lang="ts">
  import { Checkbox, ToggleGroup } from "bits-ui";
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import Check from "@lucide/svelte/icons/check";
  import Minus from "@lucide/svelte/icons/minus";
  import Plus from "@lucide/svelte/icons/plus";
  import Swords from "@lucide/svelte/icons/swords";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import DuePicker from "./DuePicker.svelte";
  import schema, { sprites, stickers, tapes, type Course, type Placed, type Quest, type Sticker } from "./schema";
  import { addDays, byDue, cleared, dayValue, daysBetween, emoji, hp, isoDay, lootFor, nextBoss, span, spriteFace, when } from "./quest";

  const doc = useDocument(schema);
  const today = isoDay(new Date());
  let selectedId = $state<string | null>(null);
  let announcement = $state("");
  let pulse = $state(0);
  let drag = $state<{ id: string; x: number; y: number } | null>(null);
  let sideDraft = $state("");

  const range = $derived(span(doc.current.courses, today));
  const boss = $derived(nextBoss(doc.current.courses, today));
  const allQuests = $derived(doc.current.courses.flatMap((course) => course.quests));
  const clearedCount = $derived(allQuests.filter(cleared).length);
  const earned = $derived(clearedCount + doc.current.sideQuests.filter((side) => side.done).length);
  const loot = $derived(Math.max(0, earned - doc.current.placed.length));
  const selected = $derived.by(() => {
    for (const course of doc.current.courses) {
      const quest = course.quests.find((item) => item.$id === selectedId);
      if (quest) return { course, quest };
    }
    return undefined;
  });
  const months = $derived.by(() => {
    const marks: { x: number; label: string }[] = [];
    const cursor = new Date(range.start);
    cursor.setDate(1);
    cursor.setMonth(cursor.getMonth() + 1);
    while (cursor.getTime() < range.end) {
      marks.push({ x: xOf(isoDay(cursor)), label: cursor.toLocaleDateString(undefined, { month: "short" }) });
      cursor.setMonth(cursor.getMonth() + 1);
    }
    return marks;
  });

  function xOf(iso: string): number { return 3 + ((dayValue(iso, today) - range.start) / (range.end - range.start)) * 94; }
  function yOf(row: number, index: number, quest: Quest): number {
    const lane = 1000 / Math.max(1, doc.current.courses.length);
    return lane * (row + 0.5) + (quest.kind === "boss" ? 0 : index % 2 ? lane * 0.16 : -lane * 0.16);
  }
  function points(course: Course, row: number) {
    return byDue(course.quests).map((quest, index) => ({ quest, x: xOf(quest.due) * 10, y: yOf(row, index, quest), up: quest.kind === "quest" && index % 2 === 0 }));
  }
  function trail(course: Course, row: number): string {
    const mid = (1000 / Math.max(1, doc.current.courses.length)) * (row + 0.5);
    const stops = [{ x: 0, y: mid }, ...points(course, row), { x: 1000, y: mid }];
    let d = `M ${stops[0]!.x} ${stops[0]!.y}`;
    for (let i = 1; i < stops.length; i++) {
      const a = stops[i - 1]!, b = stops[i]!;
      const bend = (b.x - a.x) * 0.5;
      d += ` C ${a.x + bend} ${a.y}, ${b.x - bend} ${b.y}, ${b.x} ${b.y}`;
    }
    return d;
  }
  function shortDate(iso: string): string {
    return new Date(dayValue(iso, today)).toLocaleDateString(undefined, { month: "short", day: "numeric" });
  }
  function questLabel(quest: Quest): string { return quest.title.trim() || (quest.kind === "boss" ? "Boss" : "Quest"); }
  function courseLabel(course: Course): string { return course.code.trim() || course.name.trim() || "Course"; }
  function overdue(quest: Quest): boolean { return !cleared(quest) && quest.due < today; }
  function handle(course: Course, quest: Quest, tx: { fields: typeof doc.fields }) { return tx.fields.courses.item(course.$id).quests.item(quest.$id); }

  function addCourse() {
    const used = new Set(doc.current.courses.map((course) => course.tape));
    const tape = tapes.find((item) => !used.has(item)) ?? tapes[doc.current.courses.length % tapes.length]!;
    doc.fields.courses.insert({ code: "NEW 100", name: "", tape, quests: [] });
    announcement = "Course region added.";
  }

  function removeCourse(course: Course) {
    if (selected?.course.$id === course.$id) selectedId = null;
    doc.fields.courses.remove(course.$id);
    announcement = `${courseLabel(course)} removed from the map.`;
  }

  function cycleTape(course: Course) {
    doc.at(course).tape.set(tapes[(tapes.indexOf(course.tape) + 1) % tapes.length]!);
  }

  function addQuest(course: Course, kind: Quest["kind"] = "quest") {
    const { id } = doc.at(course).quests.insert({
      title: kind === "boss" ? "Exam" : "New quest", kind, due: addDays(today, 7), done: false, maxHp: 6, hits: 0, notes: "",
    });
    selectedId = id;
    announcement = `${kind === "boss" ? "Boss" : "Quest"} added to ${courseLabel(course)}, due in 7 days.`;
  }

  function removeQuest(course: Course, quest: Quest) {
    doc.at(course).quests.remove(quest.$id);
    selectedId = null;
    announcement = `${questLabel(quest)} removed.`;
  }

  function setDone(course: Course, quest: Quest, done: boolean) {
    doc.change((tx) => {
      const item = handle(course, quest, tx);
      item.done.set(done);
      if (done) item.sticker.set(lootFor(quest.$id));
      else item.sticker.clear();
    }, { message: done ? "Clear quest" : "Reopen quest" });
    announcement = done ? `${questLabel(quest)} cleared. Sticker earned.` : `${questLabel(quest)} reopened.`;
  }

  function hit(course: Course, quest: Quest, by: 1 | -1) {
    const currentHp = hp(quest);
    if (quest.hits === null || currentHp === null) return;
    if (by > 0 && currentHp === 0) return;
    if (by < 0 && quest.hits <= 0) return;
    const defeated = by > 0 && currentHp === 1;
    doc.change((tx) => {
      const item = handle(course, quest, tx);
      if (by > 0) item.hits.increment();
      else item.hits.decrement();
      if (defeated) item.sticker.set(lootFor(quest.$id));
      else if (by < 0 && currentHp === 0) { item.sticker.clear(); item.done.set(false); }
    }, { message: by > 0 ? "Log study session" : "Undo study session" });
    pulse++;
    const left = Math.max(0, currentHp - by);
    announcement = defeated ? `K.O.! ${courseLabel(course)} ${questLabel(quest)} defeated.` : `${left} of ${quest.maxHp} HP left.`;
  }

  function setMaxHp(quest: Quest, event: Event) {
    const value = Math.round(Number((event.currentTarget as HTMLInputElement).value));
    if (Number.isFinite(value)) doc.at(quest).maxHp.set(Math.min(50, Math.max(1, value)));
  }

  function addSide() {
    const title = sideDraft.trim();
    if (!title) return;
    doc.fields.sideQuests.insert({ title, done: false });
    sideDraft = "";
  }

  function placeSticker(sticker: Sticker) {
    if (loot === 0) return;
    const count = doc.current.placed.length;
    doc.fields.placed.insert({ sticker, x: 0.2 + ((count * 0.137) % 0.6), y: 0.15 + ((count * 0.291) % 0.7), turn: ((count * 37) % 30) - 15 });
    announcement = `${sticker} sticker placed. Drag it anywhere, or press Delete to peel it off.`;
  }

  function stickerDown(event: PointerEvent, placed: Placed) {
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    drag = { id: placed.$id, x: placed.x, y: placed.y };
  }

  function stickerMove(event: PointerEvent) {
    if (!drag) return;
    const board = (event.currentTarget as HTMLElement).closest(".sq-board")!.getBoundingClientRect();
    drag = { ...drag, x: clamp((event.clientX - board.left) / board.width), y: clamp((event.clientY - board.top) / board.height) };
  }

  function stickerUp(placed: Placed) {
    if (!drag || drag.id !== placed.$id) return;
    const { x, y } = drag;
    drag = null;
    if (x === placed.x && y === placed.y) return;
    doc.change((tx) => { const item = tx.fields.placed.item(placed.$id); item.x.set(x); item.y.set(y); }, { message: "Move sticker" });
  }

  function stickerKey(event: KeyboardEvent, placed: Placed) {
    const step = event.shiftKey ? 0.05 : 0.01;
    const moves: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      doc.fields.placed.remove(placed.$id);
      announcement = "Sticker peeled off and returned to your loot.";
    } else if (moves[event.key]) {
      event.preventDefault();
      const [dx, dy] = moves[event.key]!;
      doc.change((tx) => { const item = tx.fields.placed.item(placed.$id); item.x.set(clamp(placed.x + dx)); item.y.set(clamp(placed.y + dy)); }, { message: "Move sticker" });
    }
  }

  function clamp(value: number): number { return Math.min(0.97, Math.max(0.03, value)); }

  function cycleSprite() {
    doc.fields.player.sprite.set(sprites[(sprites.indexOf(doc.current.player.sprite) + 1) % sprites.length]!);
  }
</script>

{#snippet monster(color: string, ko: boolean, seed: number)}
  <svg class="sq-monster" viewBox="0 0 64 64" aria-hidden="true" data-ko={ko}>
    <path class="sq-monster-horn" d={seed % 2 ? "M18 16 L14 4 L26 13 M46 16 L50 4 L38 13" : "M20 15 Q16 4 24 8 M44 15 Q48 4 40 8"} />
    <path class="sq-monster-body" style={`fill:${color}`}
      d={seed % 3 === 0
        ? "M10 36 C8 18 22 10 33 11 C47 12 57 22 55 38 C54 50 58 57 50 56 C44 58 42 52 36 56 C30 58 26 53 20 57 C12 58 12 50 10 36 Z"
        : seed % 3 === 1
          ? "M12 30 C12 14 24 9 34 10 C48 11 54 20 54 32 C54 44 58 54 48 55 L42 50 L36 56 L30 50 L24 56 L18 50 C10 52 12 42 12 30 Z"
          : "M9 38 C6 24 16 11 32 11 C48 11 58 22 56 36 C55 48 48 57 32 57 C18 57 11 50 9 38 Z"} />
    {#if ko}
      <path class="sq-monster-eye-x" d="M20 26 l8 8 M28 26 l-8 8 M36 26 l8 8 M44 26 l-8 8" />
    {:else}
      <circle cx="24" cy="30" r="6" fill="#fff" /><circle cx="41" cy="30" r="6" fill="#fff" />
      <circle cx="25.5" cy="31" r="2.6" /><circle cx="39.5" cy="31" r="2.6" />
    {/if}
    <path class="sq-monster-mouth" d={ko ? "M24 46 q8 -5 16 0" : "M22 43 q10 8 20 0 l-3 4 l-3 -3 l-4 4 l-3 -4 l-3 3 z"} />
  </svg>
{/snippet}

{#snippet sticker(kind: Sticker)}<span class="sq-sticker-face">{emoji[kind]}</span>{/snippet}

{#snippet board(live: boolean)}
  <section class="sq-board" aria-label="Semester map">
    <div class="sq-ruler" aria-hidden="true">
      {#each months as month (month.label + month.x)}<span style={`left:${month.x}%`}>{month.label}</span>{/each}
    </div>
    <div class="sq-labels">
      {#each doc.current.courses as course (course.$id)}
        <div class="sq-label">
          <div class="sq-tape" style={`--tape: var(--slop-${course.tape})`}>
            {#if live}
              <input class="sq-tape-code" aria-label="Course code" use:bindText={doc.at(course).code} />
              <input class="sq-tape-name" aria-label="{courseLabel(course)} name" placeholder="course name" use:bindText={doc.at(course).name} />
            {:else}
              <strong class="sq-tape-code">{course.code}</strong><span class="sq-tape-name">{course.name}</span>
            {/if}
          </div>
          {#if live}
            <div class="sq-label-tools" data-slop-export="hide">
              <button type="button" aria-label="Add quest to {courseLabel(course)}" title="Add quest" onclick={() => addQuest(course)}><Plus size={12} /></button>
              <button type="button" aria-label="Add boss to {courseLabel(course)}" title="Add exam boss" onclick={() => addQuest(course, "boss")}><Swords size={12} /></button>
              <button type="button" aria-label="Change {courseLabel(course)} tape colour" title="Change tape" class="sq-tape-swatch" style={`--tape: var(--slop-${course.tape})`} onclick={() => cycleTape(course)}></button>
              <button type="button" aria-label="Remove {courseLabel(course)}" title="Remove course" onclick={() => removeCourse(course)}><Trash2 size={11} /></button>
            </div>
          {/if}
        </div>
      {/each}
      {#if live && doc.current.courses.length < 6}
        <button type="button" class="sq-add-course" data-slop-export="hide" onclick={addCourse}><Plus size={12} /> Course</button>
      {/if}
    </div>

    <div class="sq-timeline">
      <div class="sq-today" style={`left:${xOf(today)}%`} aria-hidden="true"></div>
      <svg class="sq-trails" viewBox="0 0 1000 1000" preserveAspectRatio="none" aria-hidden="true">
        <defs><clipPath id={live ? "sq-past" : "sq-past-export"}><rect x="0" y="0" width={xOf(today) * 10} height="1000" /></clipPath></defs>
        {#each doc.current.courses as course, row (course.$id)}
          {@const d = trail(course, row)}
          <path class="sq-trail-future" d={d} />
          <path class="sq-trail-past" d={d} clip-path={`url(#${live ? "sq-past" : "sq-past-export"})`} />
        {/each}
      </svg>
      {#each doc.current.courses as course, row (course.$id)}
        {#each points(course, row) as { quest, x, y, up } (quest.$id)}
          {@const done = cleared(quest)}
          {@const hpLeft = hp(quest)}
          {@const label = `${courseLabel(course)} ${questLabel(quest)}, ${quest.kind}, due ${shortDate(quest.due)}${done ? ", cleared" : quest.kind === "boss" ? (hpLeft === null ? ", HP unavailable" : `, ${hpLeft} of ${quest.maxHp} HP`) : overdue(quest) ? ", overdue" : ""}`}
{#snippet face()}
            {#if quest.kind === "boss"}
              {@render monster(`var(--slop-${course.tape})`, done, row + quest.maxHp)}
              {#if done}<span class="sq-ko">K.O.</span>{:else if hpLeft === null}<span>HP unavailable</span>{:else}<span class="sq-hpbar"><i style={`width:${(hpLeft / quest.maxHp) * 100}%`}></i></span>{/if}
            {:else if done}
              <span class="sq-node-sticker">{@render sticker(quest.sticker ?? lootFor(quest.$id))}</span>
            {:else}
              <span class="sq-node-dot">{overdue(quest) ? "!" : ""}</span>
            {/if}
            <span class="sq-node-label"><b>{questLabel(quest)}</b><small>{shortDate(quest.due)}</small></span>
          {/snippet}
          {#if live}
            <button type="button" class="sq-node" data-up={up} data-kind={quest.kind} data-done={done} data-overdue={overdue(quest)} data-selected={selectedId === quest.$id}
            data-pulse={quest.kind === "boss" && selectedId === quest.$id ? pulse % 2 : undefined}
            style={`left:${x / 10}%; top:${y / 10}%; --tape: var(--slop-${course.tape})`}
              aria-label={label} aria-pressed={selectedId === quest.$id} onclick={() => (selectedId = selectedId === quest.$id ? null : quest.$id)}>{@render face()}</button>
          {:else}
            <div class="sq-node" data-up={up} data-kind={quest.kind} data-done={done} data-overdue={overdue(quest)} data-selected={selectedId === quest.$id}
            data-pulse={quest.kind === "boss" && selectedId === quest.$id ? pulse % 2 : undefined}
            style={`left:${x / 10}%; top:${y / 10}%; --tape: var(--slop-${course.tape})`}>{@render face()}</div>
          {/if}
        {/each}
      {/each}
      <div class="sq-player" style={`left:${xOf(today)}%`}>
        {#if live}
          <button type="button" class="sq-player-face" aria-label="Change your sprite" title="Change sprite" onclick={cycleSprite}>{spriteFace[doc.current.player.sprite]}</button>
        {:else}
          <span class="sq-player-face">{spriteFace[doc.current.player.sprite]}</span>
        {/if}
        <span class="sq-player-tag">you · today</span>
      </div>
    </div>

    {#each doc.current.placed as placed (placed.$id)}
      {@const at = drag?.id === placed.$id ? drag : placed}
      {#if live}
        <button type="button" class="sq-placed" data-dragging={drag?.id === placed.$id}
          style={`left:${at.x * 100}%; top:${at.y * 100}%; --turn:${placed.turn}deg`}
          aria-label="{placed.sticker} sticker. Arrow keys move it, Delete peels it off."
          onpointerdown={(event) => stickerDown(event, placed)} onpointermove={stickerMove}
          onpointerup={() => stickerUp(placed)} onpointercancel={() => (drag = null)}
          onkeydown={(event) => stickerKey(event, placed)}>{@render sticker(placed.sticker)}</button>
      {:else}
        <span class="sq-placed" style={`left:${placed.x * 100}%; top:${placed.y * 100}%; --turn:${placed.turn}deg`}>{@render sticker(placed.sticker)}</span>
      {/if}
    {/each}
    {#if doc.current.courses.length === 0}
      <p class="sq-empty">A blank map.<br />Add your first course to draw its trail.</p>
    {/if}
  </section>
{/snippet}

{#snippet bossReadout()}
  <div class="sq-readout" aria-live="polite">
    {#if boss}
      {@const days = daysBetween(today, boss.quest.due)}
      {@const bossHp = hp(boss.quest)}
      <span class="sq-readout-eyebrow">Next boss</span>
      <strong>{courseLabel(boss.course)} {questLabel(boss.quest)}</strong>
      <span class="sq-readout-meta">{when(days)} · <b>{bossHp === null ? "HP unavailable" : `${Math.round((bossHp / boss.quest.maxHp) * 100)}% HP`}</b></span>
    {:else}
      <span class="sq-readout-eyebrow">Next boss</span>
      <strong>None in sight</strong>
      <span class="sq-readout-meta">Every boss is down. Touch grass.</span>
    {/if}
  </div>
{/snippet}

<Slop>
  <main class="sq-page">
    <div class="sq-spiral" aria-hidden="true"></div>
    <header class="sq-head">
      <div class="sq-title">
        <h1 class="sq-sr-only">{doc.current.semester} semester map</h1>
        <input class="sq-semester" aria-label="Semester name" use:bindText={doc.fields.semester} />
        <span class="sq-subtitle">side quest · semester map</span>
      </div>
      {@render bossReadout()}
      <dl class="sq-stats">
        <div><dt>Cleared</dt><dd>{clearedCount}<small>/{allQuests.length}</small></dd></div>
        <div><dt>Loot</dt><dd>{loot}</dd></div>
      </dl>
    </header>

    <div class="sq-body">
      {@render board(true)}

      <aside class="sq-panel" data-slop-export="hide" aria-label={selected ? "Quest details" : "Side quests and loot"}>
        {#if selected}
          {@const { course, quest } = selected}
          {@const hpLeft = hp(quest)}
          <div class="sq-panel-head">
            <span class="sq-tape sq-tape-mini" style={`--tape: var(--slop-${course.tape})`}>{courseLabel(course)}</span>
            <button type="button" class="sq-icon-btn" aria-label="Close details" onclick={() => (selectedId = null)}><X size={14} /></button>
          </div>
          <textarea class="sq-quest-title" rows="2" aria-label="Quest title" use:bindText={doc.at(quest).title}></textarea>
          <ToggleGroup.Root type="single" class="sq-kind" aria-label="Quest type" value={quest.kind}
            onValueChange={(value) => { if (value === "quest" || value === "boss") doc.at(quest).kind.set(value); }}>
            <ToggleGroup.Item value="quest">Quest</ToggleGroup.Item>
            <ToggleGroup.Item value="boss">Boss</ToggleGroup.Item>
          </ToggleGroup.Root>
          <div class="sq-field"><span>Due</span><DuePicker value={quest.due} onChange={(iso) => doc.at(quest).due.set(iso)} /></div>
          <p class="sq-due-note" data-overdue={overdue(quest)}>{cleared(quest) ? "Cleared ✓ · due" : overdue(quest) ? "Overdue ·" : "Due"} {when(daysBetween(today, quest.due))}</p>

          {#if quest.kind === "boss"}
            <div class="sq-boss-card" data-pulse={pulse % 2} data-ko={hpLeft === 0}>
              <div class="sq-boss-hp">
                <span>HP</span>
                {#if hpLeft !== null}<span class="sq-hpbar sq-hpbar-big" role="meter" aria-label="Boss HP" aria-valuemin={0} aria-valuemax={quest.maxHp} aria-valuenow={hpLeft}><i style={`width:${(hpLeft / quest.maxHp) * 100}%`}></i></span>{/if}
                <b>{hpLeft === null ? "HP unavailable" : `${hpLeft}/${quest.maxHp}`}</b>
              </div>
              <button type="button" class="sq-hit" disabled={hpLeft === null || hpLeft === 0} onclick={() => hit(course, quest, 1)}>
                <Swords size={15} /> {hpLeft === 0 ? "Defeated" : "Log study session"}
              </button>
              <div class="sq-boss-row">
                <button type="button" class="sq-link" disabled={quest.hits === null || quest.hits <= 0} onclick={() => hit(course, quest, -1)}><Minus size={11} /> Undo hit</button>
                <label>Sessions <input type="number" min="1" max="50" value={quest.maxHp} onchange={(event) => setMaxHp(quest, event)} /></label>
              </div>
            </div>
          {:else}
            <label class="sq-check-row">
              <Checkbox.Root class="sq-check" checked={quest.done} onCheckedChange={(checked) => setDone(course, quest, checked)}>
                {#snippet children({ checked })}{#if checked}<Check size={13} strokeWidth={3.5} />{/if}{/snippet}
              </Checkbox.Root>
              <span>{quest.done ? "Cleared, sticker earned" : "Mark cleared"}</span>
            </label>
          {/if}
          <label class="sq-field sq-field-grow"><span>Notes</span><textarea rows="3" placeholder="chapters, links, who's in your group…" use:bindText={doc.at(quest).notes}></textarea></label>
          <button type="button" class="sq-link sq-danger" onclick={() => removeQuest(course, quest)}><Trash2 size={11} /> Remove {quest.kind}</button>
        {:else}
          <h2 class="sq-panel-title">Side quests</h2>
          <ul class="sq-sides">
            {#each doc.current.sideQuests as side (side.$id)}
              <li data-done={side.done}>
                <Checkbox.Root class="sq-check" aria-label="Side quest done" checked={side.done} onCheckedChange={(checked) => doc.at(side).done.set(checked)}>
                  {#snippet children({ checked })}{#if checked}<Check size={13} strokeWidth={3.5} />{/if}{/snippet}
                </Checkbox.Root>
                <input aria-label="Side quest" use:bindText={doc.at(side).title} />
                <button type="button" class="sq-icon-btn" aria-label="Remove side quest" onclick={() => doc.fields.sideQuests.remove(side.$id)}><X size={11} /></button>
              </li>
            {/each}
          </ul>
          <form class="sq-side-add" onsubmit={(event) => { event.preventDefault(); addSide(); }}>
            <input bind:value={sideDraft} placeholder="new side quest…" aria-label="New side quest" />
            <button type="submit" class="sq-icon-btn" aria-label="Add side quest" disabled={!sideDraft.trim()}><Plus size={13} /></button>
          </form>

          <h2 class="sq-panel-title">Loot <small>{loot} to place</small></h2>
          <div class="sq-tray" role="group" aria-label="Sticker loot">
            {#each stickers as kind}
              <button type="button" disabled={loot === 0} aria-label="Place {kind} sticker" onclick={() => placeSticker(kind)}>{@render sticker(kind)}</button>
            {/each}
          </div>
          <p class="sq-hint">{loot === 0 ? "Clear quests and side quests to earn stickers." : "Tap one to slap it on your map, then drag it anywhere."}</p>
          <p class="sq-hint">Tap a node to open it. Log study sessions to knock down exam bosses before the date.</p>
        {/if}
      </aside>
    </div>
    <p class="sq-sr-only" role="status" aria-live="polite">{announcement}</p>
  </main>

  {#snippet exportView()}
    <main class="sq-page sq-export">
      <div class="sq-spiral" aria-hidden="true"></div>
      <header class="sq-head">
        <div class="sq-title"><h1 class="sq-semester">{doc.current.semester}</h1><span class="sq-subtitle">side quest · semester map</span></div>
        {@render bossReadout()}
        <dl class="sq-stats"><div><dt>Cleared</dt><dd>{clearedCount}<small>/{allQuests.length}</small></dd></div><div><dt>Stickers</dt><dd>{earned}</dd></div></dl>
      </header>
      <div class="sq-body">
        {@render board(false)}
        {#if doc.current.sideQuests.length}
          <aside class="sq-export-sides"><h2 class="sq-panel-title">Side quests</h2><ul>{#each doc.current.sideQuests as side (side.$id)}<li data-done={side.done}>{side.done ? "☑" : "☐"} {side.title}</li>{/each}</ul></aside>
        {/if}
      </div>
    </main>
  {/snippet}

  {#snippet icon()}
    <div class="sq-icon" aria-hidden="true">
      <div class="sq-icon-map">
        <svg viewBox="0 0 400 400">
          <path class="sq-icon-trail" d="M40 330 C120 330 90 230 170 230 C250 230 220 130 300 120" />
          <circle cx="40" cy="330" r="16" fill="var(--slop-mint)" stroke="var(--slop-ink)" stroke-width="7" />
          <circle cx="170" cy="230" r="16" fill="var(--slop-pink)" stroke="var(--slop-ink)" stroke-width="7" />
        </svg>
        <div class="sq-icon-boss">{@render monster("var(--slop-boss)", false, 0)}</div>
        <span class="sq-icon-star">⭐</span>
      </div>
    </div>
  {/snippet}
</Slop>
