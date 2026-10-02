<script lang="ts">
  import doc from "./schema";
  import { initials, weekTotals } from "./chores";
  import Wheel from "./Wheel.svelte";

  const totals = $derived(weekTotals(doc.current.people, doc.current.chores));
  const personById = (id: string | undefined) => doc.current.people.find((person) => person.$id === id);
</script>

<article class="door export" aria-label="Exported chore chart">
  <header class="head">
    <h1 class="title">{doc.current.title || "Our place"}</h1>
    <span class="week">Week {(doc.current.weeks ?? 0) + 1}</span>
  </header>
  <section class="board">
    <Wheel people={doc.current.people} {totals} />
    <ul class="people">
      {#each doc.current.people as person, index (person.$id)}
        <li class="person">
          <span class="magnet" data-tone={person.tone}>{initials(person.name)}</span>
          <div class="pname"><strong>{person.name || "Unnamed"}</strong><span>{totals[index]?.done ?? 0}/{totals[index]?.assigned ?? 0} this week · <b>{person.score ?? 0}</b> all-time</span></div>
        </li>
      {/each}
    </ul>
  </section>
  <section class="chores">
    <ul class="notes">
      {#each doc.current.chores as chore (chore.$id)}
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
  {#if doc.current.rules.trim()}
    <aside class="rules"><span class="pin" aria-hidden="true"></span><p>{doc.current.rules}</p></aside>
  {/if}
</article>
