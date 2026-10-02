<script lang="ts">
import Check from "@lucide/svelte/icons/check";
import doc from "./schema";
const completedAgenda = $derived(doc.current.agenda.filter((item) => item.done).length);
const completedActions = $derived(doc.current.actions.filter((item) => item.done).length);
const data = $derived(doc.current);
</script>

<article class="exportMemo" aria-label="Exported meeting memo {data.title}">
  <header class="letterhead">
    <div class="metaRow">
      <span class="badge"><span class="badgeDot"></span>Meeting Memo</span>
      <div class="dateRow">
        <span class="metaField">{data.date}</span>
        {#if data.date.trim() && data.time.trim()}<span aria-hidden="true">·</span>{/if}
        <span class="metaField">{data.time}</span>
      </div>
    </div>
    <h1 class="title">{data.title.trim() || "Untitled meeting"}</h1>
    <div class="attendees">
      <span class="attendeesLabel">Attendees</span>
      {#each data.attendees.filter((person) => person.trim()) as person}
        <span class="pill">{person}</span>
      {:else}
        <span class="pill">None listed</span>
      {/each}
    </div>
  </header>

  <div class="body">
    <div class="column">
      <section class="section" aria-labelledby="export-agenda">
        <div class="sectionHeader">
          <h2 id="export-agenda" class="sectionTitle">Agenda <span class="sectionCount">({completedAgenda}/{data.agenda.length})</span></h2>
        </div>
        <ul class="list">
          {#each data.agenda as item (item.$id)}
            <li class="agendaItem" data-done={item.done}>
              <span data-checkbox-root data-state={item.done ? "checked" : "unchecked"}>{#if item.done}<Check size={10} strokeWidth={3} />{/if}</span>
              <span class="rowText">{item.text.trim() || "Untitled topic"}</span>
            </li>
          {:else}
            <li class="empty">No topics yet.</li>
          {/each}
        </ul>
      </section>

      <section class="decisions" aria-labelledby="export-decisions">
        <div class="decisionsHeader">
          <h2 id="export-decisions" class="decisionsTitle"><span class="decisionsDot"></span>Decisions</h2>
        </div>
        <ul class="list">
          {#each data.decisions as item (item.$id)}
            <li class="decisionItem">
              <span class="decisionBullet" aria-hidden="true">▪</span>
              <span class="rowText">{item.text.trim() || "Untitled decision"}</span>
            </li>
          {:else}
            <li class="empty">Nothing decided yet.</li>
          {/each}
        </ul>
      </section>
    </div>

    <div class="column">
      <section class="section" aria-labelledby="export-notes">
        <div class="sectionHeader"><h2 id="export-notes" class="sectionTitle">Notes</h2></div>
        {#if data.notes.trim()}
          <p class="notesText">{data.notes}</p>
        {:else}
          <p class="empty">No notes captured.</p>
        {/if}
      </section>

      <section class="section" aria-labelledby="export-actions">
        <div class="sectionHeader">
          <h2 id="export-actions" class="sectionTitle">Actions <span class="sectionCount">({completedActions}/{data.actions.length})</span></h2>
        </div>
        <ul class="list">
          {#each data.actions as item (item.$id)}
            <li class="actionItem" data-done={item.done}>
              <span data-checkbox-root data-state={item.done ? "checked" : "unchecked"}>{#if item.done}<Check size={10} strokeWidth={3} />{/if}</span>
              <span class="rowText">{item.text.trim() || "Untitled action"}</span>
              <span class="owner">{item.owner.trim() || "Unassigned"}</span>
            </li>
          {:else}
            <li class="empty">No follow-ups yet.</li>
          {/each}
        </ul>
      </section>
    </div>
  </div>

  <footer class="footer">
    <span>{data.agenda.length} agenda · {data.decisions.length} decisions</span>
    <span>{completedActions} of {data.actions.length} actions done</span>
  </footer>
</article>
