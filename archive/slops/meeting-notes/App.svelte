<script lang="ts">
  import { Button, Checkbox } from "bits-ui";
  import Check from "@lucide/svelte/icons/check";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import { Slop, bindText, bindValue, useDocument } from "@hitslop/document/svelte";
  import schema from "./schema";

  const doc = useDocument(schema);
  let newAttendee = $state("");
  const completedAgenda = $derived(doc.current.agenda.filter((item) => item.done).length);
  const completedActions = $derived(doc.current.actions.filter((item) => item.done).length);

  function addAttendee() {
    const name = newAttendee.trim();
    if (!name) return;
    doc.fields.attendees.insert(name);
    newAttendee = "";
  }
</script>

<Slop>
  <main class="canvas" data-slop-selection="none">
    <article class="memo" aria-label="Meeting memo {doc.current.title}">
      <header class="letterhead">
        <div class="metaRow">
          <span class="badge"><span class="badgeDot"></span>Meeting Memo</span>
          <div class="dateRow">
            <label><span class="srOnly">Meeting date</span><input class="metaField" use:bindValue={doc.fields.date} /></label>
            <span aria-hidden="true">·</span>
            <label><span class="srOnly">Meeting time or location</span><input class="metaField" use:bindValue={doc.fields.time} /></label>
          </div>
        </div>
        <input class="title" aria-label="Meeting title" placeholder="Meeting title" use:bindText={doc.fields.title} />
        <div class="attendees">
          <span class="attendeesLabel">Attendees</span>
          {#each doc.current.attendees as person, index}
            <span class="pill">
              <span>{person}</span>
              <button class="pillRemove" data-slop-export="hide" type="button" aria-label="Remove {person}" onclick={() => doc.fields.attendees.remove(index)}><X size={11} strokeWidth={2} /></button>
            </span>
          {/each}
          <input
            class="attendeeInput"
            data-slop-export="hide"
            aria-label="Add attendee"
            placeholder="+ Add person"
            bind:value={newAttendee}
            onkeydown={(event) => { if (event.key === "Enter") { event.preventDefault(); addAttendee(); } }}
          />
        </div>
      </header>

      <div class="body">
        <div class="column">
          <section class="section" aria-labelledby="agenda-heading">
            <div class="sectionHeader">
              <h2 id="agenda-heading" class="sectionTitle">Agenda <span class="sectionCount">({completedAgenda}/{doc.current.agenda.length})</span></h2>
              <Button.Root class="add" data-slop-export="hide" aria-label="Add agenda topic" onclick={() => doc.fields.agenda.insert({ text: "", done: false })}><Plus size={14} strokeWidth={1.8} /></Button.Root>
            </div>
            <ul class="list">
              {#each doc.current.agenda as item, index (item.$id)}
                <li class="agendaItem" data-done={item.done}>
                  <Checkbox.Root checked={item.done} onCheckedChange={(checked) => doc.at(item).done.set(checked === true)} aria-label="Mark {item.text || 'agenda topic'} {item.done ? 'open' : 'done'}">
                    {#snippet children({ checked })}{#if checked}<Check size={10} strokeWidth={3} />{/if}{/snippet}
                  </Checkbox.Root>
                  <input class="rowText" aria-label="Agenda topic {index + 1}" placeholder="Agenda topic" use:bindText={doc.at(item).text} />
                  <button class="remove" data-slop-export="hide" type="button" aria-label="Delete agenda topic {index + 1}" onclick={() => doc.fields.agenda.remove(item.$id)}><Trash2 size={13} strokeWidth={1.7} /></button>
                </li>
              {:else}
                <li class="empty">No topics yet.</li>
              {/each}
            </ul>
          </section>

          <section class="decisions" aria-labelledby="decisions-heading">
            <div class="decisionsHeader">
              <h2 id="decisions-heading" class="decisionsTitle"><span class="decisionsDot"></span>Decisions</h2>
              <Button.Root class="add" data-slop-export="hide" aria-label="Add decision" onclick={() => doc.fields.decisions.insert({ text: "" })}><Plus size={14} strokeWidth={1.8} /></Button.Root>
            </div>
            <ul class="list">
              {#each doc.current.decisions as item, index (item.$id)}
                <li class="decisionItem">
                  <span class="decisionBullet" aria-hidden="true">▪</span>
                  <input class="rowText" aria-label="Decision {index + 1}" placeholder="Agreed decision" use:bindText={doc.at(item).text} />
                  <button class="remove" data-slop-export="hide" type="button" aria-label="Delete decision {index + 1}" onclick={() => doc.fields.decisions.remove(item.$id)}><Trash2 size={13} strokeWidth={1.7} /></button>
                </li>
              {:else}
                <li class="empty">Nothing decided yet.</li>
              {/each}
            </ul>
          </section>
        </div>

        <div class="column">
          <section class="section" aria-labelledby="notes-heading">
            <div class="sectionHeader"><h2 id="notes-heading" class="sectionTitle">Notes</h2></div>
            <textarea class="notes" aria-label="Meeting notes" placeholder="Capture discussion, context, and references…" use:bindText={doc.fields.notes}></textarea>
          </section>

          <section class="section" aria-labelledby="actions-heading">
            <div class="sectionHeader">
              <h2 id="actions-heading" class="sectionTitle">Actions <span class="sectionCount">({completedActions}/{doc.current.actions.length})</span></h2>
              <Button.Root class="add" data-slop-export="hide" aria-label="Add action item" onclick={() => doc.fields.actions.insert({ text: "", owner: "", done: false })}><Plus size={14} strokeWidth={1.8} /></Button.Root>
            </div>
            <ul class="list">
              {#each doc.current.actions as item, index (item.$id)}
                <li class="actionItem" data-done={item.done}>
                  <Checkbox.Root checked={item.done} onCheckedChange={(checked) => doc.at(item).done.set(checked === true)} aria-label="Mark {item.text || 'action'} {item.done ? 'open' : 'complete'}">
                    {#snippet children({ checked })}{#if checked}<Check size={10} strokeWidth={3} />{/if}{/snippet}
                  </Checkbox.Root>
                  <input class="rowText" aria-label="Action {index + 1}" placeholder="Follow-up action" use:bindText={doc.at(item).text} />
                  <input class="owner" aria-label="Owner for action {index + 1}" placeholder="Owner" use:bindText={doc.at(item).owner} />
                  <button class="remove" data-slop-export="hide" type="button" aria-label="Delete action {index + 1}" onclick={() => doc.fields.actions.remove(item.$id)}><Trash2 size={13} strokeWidth={1.7} /></button>
                </li>
              {:else}
                <li class="empty">No follow-ups yet.</li>
              {/each}
            </ul>
          </section>
        </div>
      </div>

      <footer class="footer">
        <span>{doc.current.agenda.length} agenda · {doc.current.decisions.length} decisions</span>
        <span>{completedActions} of {doc.current.actions.length} actions done</span>
      </footer>
    </article>
  </main>

  {#snippet exportView()}
    {@const data = doc.current}
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
  {/snippet}

  {#snippet icon()}
    <div class="iconSurface" aria-hidden="true">
      <article class="iconTile">
        <div class="iconSheet">
          <span class="iconClip"></span>
          <div class="iconHeader"></div>
          <span class="iconRuleA"></span>
          <span class="iconRuleB"></span>
          <div class="iconCard">
            <span class="iconCardDot"></span>
            <span class="iconCardLine"></span>
          </div>
        </div>
      </article>
    </div>
  {/snippet}
</Slop>
