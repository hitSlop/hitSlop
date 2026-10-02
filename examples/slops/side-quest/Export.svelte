<script lang="ts">
  import doc from "./schema";
  import BossReadout from "./BossReadout.svelte";
  import Monster from "./Monster.svelte";
  import StickerFace from "./StickerFace.svelte";
  import { courseLabel, mapGeometry, questLabel } from "./map";
  import { cleared, hp, isoDay, lootFor, spriteFace } from "./quest";

  const today = isoDay(new Date());
  const { xOf, points, trail, months, shortDate, overdue } = $derived(mapGeometry(doc.current.courses, today));
  const allQuests = $derived(doc.current.courses.flatMap((course) => course.quests));
  const clearedCount = $derived(allQuests.filter(cleared).length);
  const earned = $derived(clearedCount + doc.current.sideQuests.filter((side) => side.done).length);
</script>

<main class="sq-page sq-export">
  <div class="sq-spiral" aria-hidden="true"></div>
  <header class="sq-head">
    <div class="sq-title"><h1 class="sq-semester">{doc.current.semester}</h1><span class="sq-subtitle">side quest · semester map</span></div>
    <BossReadout />
    <dl class="sq-stats"><div><dt>Cleared</dt><dd>{clearedCount}<small>/{allQuests.length}</small></dd></div><div><dt>Stickers</dt><dd>{earned}</dd></div></dl>
  </header>
  <div class="sq-body">
    {@render board()}
    {#if doc.current.sideQuests.length}
      <aside class="sq-export-sides"><h2 class="sq-panel-title">Side quests</h2><ul>{#each doc.current.sideQuests as side (side.$id)}<li data-done={side.done}>{side.done ? "☑" : "☐"} {side.title}</li>{/each}</ul></aside>
    {/if}
  </div>
</main>

{#snippet board()}
  <section class="sq-board" aria-label="Semester map">
    <div class="sq-ruler" aria-hidden="true">
      {#each months as month (month.label + month.x)}<span style={`left:${month.x}%`}>{month.label}</span>{/each}
    </div>
    <div class="sq-labels">
      {#each doc.current.courses as course (course.$id)}
        <div class="sq-label">
          <div class="sq-tape" style={`--tape: var(--slop-${course.tape})`}>
              <strong class="sq-tape-code">{course.code}</strong><span class="sq-tape-name">{course.name}</span>
          </div>
        </div>
      {/each}
    </div>

    <div class="sq-timeline">
      <div class="sq-today" style={`left:${xOf(today)}%`} aria-hidden="true"></div>
      <svg class="sq-trails" viewBox="0 0 1000 1000" preserveAspectRatio="none" aria-hidden="true">
        <defs><clipPath id="sq-past-export"><rect x="0" y="0" width={xOf(today) * 10} height="1000" /></clipPath></defs>
        {#each doc.current.courses as course, row (course.$id)}
          {@const d = trail(course, row)}
          <path class="sq-trail-future" d={d} />
          <path class="sq-trail-past" d={d} clip-path={`url(#sq-past-export)`} />
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
            <div class="sq-node" data-up={up} data-kind={quest.kind} data-done={done} data-overdue={overdue(quest)}
            style={`left:${x / 10}%; top:${y / 10}%; --tape: var(--slop-${course.tape})`}>{@render face()}</div>
        {/each}
      {/each}
      <div class="sq-player" style={`left:${xOf(today)}%`}>
          <span class="sq-player-face">{spriteFace[doc.current.player.sprite]}</span>
        <span class="sq-player-tag">you · today</span>
      </div>
    </div>

    {#each doc.current.placed as placed (placed.$id)}
        <span class="sq-placed" style={`left:${placed.x * 100}%; top:${placed.y * 100}%; --turn:${placed.turn}deg`}><StickerFace kind={placed.sticker} /></span>
    {/each}
    {#if doc.current.courses.length === 0}
      <p class="sq-empty">A blank map.<br />Add your first course to draw its trail.</p>
    {/if}
  </section>
{/snippet}
