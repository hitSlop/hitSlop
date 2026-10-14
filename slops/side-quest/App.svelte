<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { Checkbox, ToggleGroup } from "bits-ui";
  import { bindText, draft, getDocument } from "hitslop/svelte";
  import Check from "@lucide/svelte/icons/check";
  import Minus from "@lucide/svelte/icons/minus";
  import Plus from "@lucide/svelte/icons/plus";
  import Swords from "@lucide/svelte/icons/swords";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import BossReadout from "./BossReadout.svelte";
  import DuePicker from "./DuePicker.svelte";
  import Monster from "./Monster.svelte";
  import StickerFace from "./StickerFace.svelte";
  import schema, { sprites, stickers } from "./schema";
  import { courseLabel, mapGeometry, questLabel } from "./map";
  import {
    addCourse as addCourseEdit, addDays, addQuest as addQuestEdit, addSideQuest, cleared, cycleSprite as cycleSpriteEdit,
    cycleTape as cycleTapeEdit, daysBetween, hit as hitEdit, hp, isoDay, lootFor, moveSticker, peelSticker, placeSticker as placeStickerEdit,
    questView, removeCourse as removeCourseEdit, removeQuest as removeQuestEdit, setDone as setDoneEdit, setDue, setMaxHp as setMaxHpEdit,
    setSideDone, spriteFace, when, type Course, type Kind, type Placed, type Quest, type Sticker,
  } from "./quest";

  const doc = getDocument(schema);
  const today = isoDay(new Date());
  let selectedId = $state<string | null>(null);
  let announcement = $state("");
  let pulse = $state(0);
  let drag = $state<{ id: string; x: number; y: number } | null>(null);
  const newSide = draft((title) => doc.change((tx) => addSideQuest(tx, title)));

  const { xOf, points, trail, months, shortDate, overdue } = $derived(mapGeometry(doc.current.courses, today));
  const { allQuests, clearedCount, loot, boss } = $derived(questView(doc.current, today));
  const selected = $derived.by(() => {
    for (const course of doc.current.courses) {
      const quest = course.quests.find((item) => item.$id === selectedId);
      if (quest) return { course, quest };
    }
    return undefined;
  });

  async function addCourse() {
    await doc.change((tx) => addCourseEdit(tx, doc.current));
    announcement = "Course region added.";
  }

  function removeCourse(course: Course) {
    if (selected?.course.$id === course.$id) selectedId = null;
    doc.change((tx) => removeCourseEdit(tx, course));
    announcement = `${courseLabel(course)} removed from the map.`;
  }

  function cycleTape(course: Course) {
    doc.change((tx) => cycleTapeEdit(tx, course));
  }

  async function addQuest(course: Course, kind: Kind = "quest") {
    const { id } = await doc.change((tx) => addQuestEdit(tx, course, kind, addDays(today, 7)));
    selectedId = id;
    announcement = `${kind === "boss" ? "Boss" : "Quest"} added to ${courseLabel(course)}, due in 7 days.`;
  }

  function removeQuest(quest: Quest) {
    doc.change((tx) => removeQuestEdit(tx, doc.current, quest));
    selectedId = null;
    announcement = `${questLabel(quest)} removed.`;
  }

  function setDone(quest: Quest, done: boolean) {
    doc.change((tx) => setDoneEdit(tx, doc.current, quest, done));
    announcement = done ? `${questLabel(quest)} cleared. Sticker earned.` : `${questLabel(quest)} reopened.`;
  }

  function hit(course: Course, quest: Quest, by: 1 | -1) {
    const currentHp = hp(quest);
    if (quest.hits === null || currentHp === null) return;
    if (by > 0 && currentHp === 0) return;
    if (by < 0 && quest.hits <= 0) return;
    const defeated = by > 0 && currentHp === 1;
    doc.change((tx) => hitEdit(tx, doc.current, quest, by));
    pulse++;
    const left = Math.max(0, currentHp - by);
    announcement = defeated ? `K.O.! ${courseLabel(course)} ${questLabel(quest)} defeated.` : `${left} of ${quest.maxHp} HP left.`;
  }

  function setMaxHp(quest: Quest, event: Event) {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    doc.change((tx) => setMaxHpEdit(tx, quest, value));
  }

  async function placeSticker(sticker: Sticker) {
    if (loot === 0) return;
    await doc.change((tx) => placeStickerEdit(tx, doc.current, sticker));
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
    doc.change((tx) => moveSticker(tx, placed, x, y));
  }

  function stickerKey(event: KeyboardEvent, placed: Placed) {
    const step = event.shiftKey ? 0.05 : 0.01;
    const moves: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      doc.change((tx) => peelSticker(tx, placed));
      announcement = "Sticker peeled off and returned to your loot.";
    } else if (moves[event.key]) {
      event.preventDefault();
      const [dx, dy] = moves[event.key]!;
      doc.change((tx) => moveSticker(tx, placed, clamp(placed.x + dx), clamp(placed.y + dy)));
    }
  }

  function clamp(value: number): number { return Math.min(0.97, Math.max(0.03, value)); }

  function cycleSprite() {
    doc.change((tx) => cycleSpriteEdit(tx, doc.current));
  }
</script>

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
              <Monster color={`var(--slop-${course.tape})`} ko={done} seed={row + quest.maxHp} />
              {#if done}<span class="sq-ko">K.O.</span>{:else if hpLeft === null}<span>HP unavailable</span>{:else}<span class="sq-hpbar"><i style={`width:${(hpLeft / quest.maxHp) * 100}%`}></i></span>{/if}
            {:else if done}
              <span class="sq-node-sticker"><StickerFace kind={quest.sticker ?? lootFor(quest.$id)} /></span>
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
          onkeydown={(event) => stickerKey(event, placed)}><StickerFace kind={placed.sticker} /></button>
      {:else}
        <span class="sq-placed" style={`left:${placed.x * 100}%; top:${placed.y * 100}%; --turn:${placed.turn}deg`}><StickerFace kind={placed.sticker} /></span>
      {/if}
    {/each}
    {#if doc.current.courses.length === 0}
      <p class="sq-empty">A blank map.<br />Add your first course to draw its trail.</p>
    {/if}
  </section>
{/snippet}

  <main class="sq-page">
    <div class="sq-spiral" aria-hidden="true"></div>
    <header class="sq-head">
      <div class="sq-title">
        <h1 class="sq-sr-only">{doc.current.semester} semester map</h1>
        <input class="sq-semester" aria-label="Semester name" use:bindText={doc.fields.semester} />
        <span class="sq-subtitle">side quest · semester map</span>
      </div>
      <BossReadout {boss} {today} />
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
          <div class="sq-field"><span>Due</span><DuePicker value={quest.due} onChange={(iso) => doc.change((tx) => setDue(tx, doc.current, quest, iso))} /></div>
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
              <Checkbox.Root class="sq-check" checked={quest.done} onCheckedChange={(checked) => setDone(quest, checked === true)}>
                {#snippet children({ checked })}{#if checked}<Check size={13} strokeWidth={3.5} />{/if}{/snippet}
              </Checkbox.Root>
              <span>{quest.done ? "Cleared, sticker earned" : "Mark cleared"}</span>
            </label>
          {/if}
          <label class="sq-field sq-field-grow"><span>Notes</span><textarea rows="3" placeholder="chapters, links, who's in your group…" use:bindText={doc.at(quest).notes}></textarea></label>
          <button type="button" class="sq-link sq-danger" onclick={() => removeQuest(quest)}><Trash2 size={11} /> Remove {quest.kind}</button>
        {:else}
          <h2 class="sq-panel-title">Side quests</h2>
          <ul class="sq-sides">
            {#each doc.current.sideQuests as side (side.$id)}
              <li data-done={side.done}>
                <Checkbox.Root class="sq-check" aria-label="Side quest done" checked={side.done} onCheckedChange={(checked) => doc.change((tx) => setSideDone(tx, side, checked === true))}>
                  {#snippet children({ checked })}{#if checked}<Check size={13} strokeWidth={3.5} />{/if}{/snippet}
                </Checkbox.Root>
                <input aria-label="Side quest" use:bindText={doc.at(side).title} />
                <button type="button" class="sq-icon-btn" aria-label="Remove side quest" onclick={() => doc.fields.sideQuests.remove(side.$id)}><X size={11} /></button>
              </li>
            {/each}
          </ul>
          <form class="sq-side-add" onsubmit={newSide.submit}>
            <input bind:value={newSide.value} placeholder="new side quest…" aria-label="New side quest" />
            <button type="submit" class="sq-icon-btn" aria-label="Add side quest" disabled={!newSide.ready}><Plus size={13} /></button>
          </form>

          <h2 class="sq-panel-title">Loot <small>{loot} to place</small></h2>
          <div class="sq-tray" role="group" aria-label="Sticker loot">
            {#each stickers as kind (kind)}
              <button type="button" disabled={loot === 0} aria-label="Place {kind} sticker" onclick={() => placeSticker(kind)}><StickerFace kind={kind} /></button>
            {/each}
          </div>
          <p class="sq-hint">{loot === 0 ? "Clear quests and side quests to earn stickers." : "Tap one to slap it on your map, then drag it anywhere."}</p>
          <p class="sq-hint">Tap a node to open it. Log study sessions to knock down exam bosses before the date.</p>
        {/if}
      </aside>
    </div>
    <p class="sq-sr-only" role="status" aria-live="polite">{announcement}</p>
  </main>
