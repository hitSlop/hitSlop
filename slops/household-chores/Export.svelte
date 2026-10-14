<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { householdView, initials } from "./chores";
  import Wheel from "./Wheel.svelte";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  const { totals, week, personById } = $derived(householdView(snapshot));
</script>

<article class="door export" aria-label="Exported chore chart">
  <header class="head">
    <h1 class="title">{snapshot.title || "Our place"}</h1>
    <span class="week">Week {week}</span>
  </header>
  <section class="board">
    <Wheel people={snapshot.people} {totals} />
    <ul class="people">
      {#each (preview ? (snapshot.people).slice(0, 4) : snapshot.people) as person, index (person.$id)}
        <li class="person">
          <span class="magnet" data-tone={person.tone}>{initials(person.name)}</span>
          <div class="pname"><strong>{person.name || "Unnamed"}</strong><span>{totals[index]?.done ?? 0}/{totals[index]?.assigned ?? 0} this week · <b>{person.score ?? 0}</b> all-time</span></div>
        </li>
      {/each}
    </ul>
  </section>
  <section class="chores">
    <ul class="notes">
      {#each (preview ? (snapshot.chores).slice(0, 5) : snapshot.chores) as chore (chore.$id)}
        {@const owner = personById(chore.assignee)}
        <li class="note" data-done={chore.done}>
          <span class="pin" aria-hidden="true"></span>
          <div class="note-top">
            <span class="tick" data-state={chore.done ? "checked" : "unchecked"}>{chore.done ? "✓" : ""}</span>
            <span class="chore-name">{chore.name || "Untitled chore"}</span>
          </div>
          <div class="note-meta">
            <span class="chip who"><span class="magnet" data-small data-tone={owner?.tone ?? "none"}>{owner ? initials(owner.name) : "–"}</span>{owner?.name || "Nobody"}</span>
            <span class="chip">{chore.day === "any" ? "Any day" : chore.day[0]!.toUpperCase() + chore.day.slice(1)}</span>
            <span class="chip">{chore.points} pt{chore.points === 1 ? "" : "s"}</span>
          </div>
        </li>
      {:else}
        <li class="empty"><h3>No chores yet.</h3></li>
      {/each}
    </ul>
  </section>
  {#if snapshot.rules.trim()}
    <aside class="rules"><span class="pin" aria-hidden="true"></span><p>{snapshot.rules}</p></aside>
  {/if}
</article>
