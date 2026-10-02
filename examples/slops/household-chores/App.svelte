<script lang="ts">
  import { bindText } from "@hitslop/document/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { Select, Checkbox, Button } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import Check from "@lucide/svelte/icons/check";
  import Repeat from "@lucide/svelte/icons/repeat";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import doc, { days, tones, type Chore, type Person } from "./schema";
  import { initials, nextPerson, weekTotals } from "./chores";
  import Wheel from "./Wheel.svelte";

  const dayItems = days.map((value) => ({ value, label: value === "any" ? "Any day" : value.toUpperCase().slice(0, 1) + value.slice(1) }));
  let personDraft = $state("");
  let choreDraft = $state("");

  const totals = $derived(weekTotals(doc.current.people, doc.current.chores));
  const open = $derived(doc.current.chores.filter((chore) => !chore.done).length);
  const flipMs = $derived(prefersReducedMotion.current ? 0 : 200);
  const week = $derived((doc.current.weeks ?? 0) + 1);

  const personById = (id: string | undefined) => doc.current.people.find((person) => person.$id === id);
  function isDay(value: string): value is (typeof days)[number] {
    return days.some((day) => day === value);
  }

  function addPerson() {
    const name = personDraft.trim();
    if (!name) return;
    doc.fields.people.insert({ name, tone: tones[doc.current.people.length % tones.length]!, score: 0 });
    personDraft = "";
  }

  function removePerson(person: Person) {
    doc.change((tx) => {
      for (const chore of doc.current.chores) {
        if (chore.assignee === person.$id) tx.at(chore).assignee.clear();
        if (chore.doneBy === person.$id) tx.at(chore).doneBy.clear();
      }
      tx.fields.people.remove(person.$id);
    });
  }

  function addChore() {
    const name = choreDraft.trim();
    if (!name) return;
    // Hand a new chore to whoever has the lightest load.
    const lightest = [...totals].sort((a, b) => a.assigned - b.assigned)[0];
    doc.fields.chores.insert({ name, day: "any", points: 1, rotates: true, done: false, ...(lightest ? { assignee: lightest.id } : {}) });
    choreDraft = "";
  }

  function toggleDone(chore: Chore, done: boolean) {
    doc.change((tx) => {
      const row = tx.at(chore);
      const scorer = personById(done ? chore.assignee : chore.doneBy);
      row.done.set(done);
      if (done && chore.assignee) row.doneBy.set(chore.assignee);
      if (!done) row.doneBy.clear();
      if (scorer) tx.at(scorer).score.increment(done ? chore.points : -chore.points);
    });
  }

  const unassigned = $derived(doc.current.chores.filter((chore) => !personById(chore.assignee)));

  // Hand each unassigned chore to whoever currently carries the fewest points.
  function dealOut() {
    const load = new Map(totals.map((t) => [t.id, t.assigned]));
    doc.change((tx) => {
      for (const chore of [...unassigned].sort((a, b) => b.points - a.points)) {
        const [id] = [...load.entries()].sort((a, b) => a[1] - b[1])[0] ?? [];
        if (!id) return;
        tx.at(chore).assignee.set(id);
        load.set(id, (load.get(id) ?? 0) + chore.points);
      }
    });
  }

  function newWeek() {
    doc.change((tx) => {
      for (const chore of doc.current.chores) {
        const row = tx.at(chore);
        if (chore.done) { row.done.set(false); row.doneBy.clear(); }
        if (chore.rotates) {
          const next = nextPerson(doc.current.people, chore.assignee);
          if (next) row.assignee.set(next);
        }
      }
      tx.fields.weeks.increment(1);
    });
  }
</script>

{#snippet magnet(person: Person | undefined, small = false)}
  <span class="magnet" data-tone={person?.tone ?? "none"} data-small={small ? "" : undefined} aria-hidden="true">{person ? initials(person.name) : "–"}</span>
{/snippet}

<main class="door" data-slop-selection="none" aria-label="Household chores">
  <header class="head">
    <input class="title" aria-label="Household name" placeholder="Our place" use:bindText={doc.fields.title} />
    <div class="weekbar">
      <span class="week">Week {week}</span>
      <Button.Root class="newweek" onclick={newWeek} aria-label="Start a new week: clear done marks and rotate chores"><RotateCw size={15} />New week</Button.Root>
    </div>
  </header>

  <section class="board" aria-label="This week">
    <Wheel people={doc.current.people} {totals} />
    <ul class="people">
      {#each doc.current.people as person, index (person.$id)}
        {@const t = totals[index]}
        <li class="person" animate:flip={{ duration: flipMs }}>
          {@render magnet(person)}
          <div class="pname">
            <input aria-label="Name" placeholder="Name" use:bindText={doc.at(person).name} />
            <span>{t?.done ?? 0}/{t?.assigned ?? 0} this week · <b>{person.score ?? 0}</b> all-time</span>
          </div>
          <button class="remove" aria-label={`Remove ${person.name || "person"}`} onclick={() => removePerson(person)}><X size={14} /></button>
        </li>
      {/each}
      <li class="person add-person">
        <form onsubmit={(event) => { event.preventDefault(); addPerson(); }}>
          <input bind:value={personDraft} aria-label="New person" placeholder="Add a person" />
          <Button.Root class="add" type="submit" aria-label="Add person" disabled={!personDraft.trim()}><Plus size={16} /></Button.Root>
        </form>
      </li>
    </ul>
  </section>

  <section class="chores" aria-label="Chores">
    <div class="chores-head">
      <h2>{open ? `${open} still to do` : "All done this week"}</h2>
      {#if unassigned.length && doc.current.people.length}
        <Button.Root class="deal" onclick={dealOut}>Deal out {unassigned.length} unassigned</Button.Root>
      {/if}
    </div>
    <ul class="notes">
      {#each doc.current.chores as chore (chore.$id)}
        {@const row = doc.at(chore)}
        {@const owner = personById(chore.assignee)}
        <li class="note" data-done={chore.done} animate:flip={{ duration: flipMs }}>
          <span class="pin" aria-hidden="true"></span>
          <div class="note-top">
            <Checkbox.Root class="tick" checked={chore.done} onCheckedChange={(checked) => toggleDone(chore, checked === true)} aria-label={`${chore.name || "Chore"}: mark ${chore.done ? "not done" : "done"}`}>
              {#snippet children({ checked })}{#if checked}<Check size={16} strokeWidth={3} />{/if}{/snippet}
            </Checkbox.Root>
            <input class="chore-name" aria-label="Chore" placeholder="Chore" use:bindText={row.name} />
            <button class="remove" aria-label={`Remove ${chore.name || "chore"}`} onclick={() => doc.fields.chores.remove(chore.$id)}><X size={14} /></button>
          </div>
          <div class="note-meta">
            <Select.Root type="single" value={chore.assignee ?? ""} items={[{ value: "", label: "Nobody" }, ...doc.current.people.map((p) => ({ value: p.$id, label: p.name || "Unnamed" }))]}
              onValueChange={(value) => { if (value) row.assignee.set(value); else row.assignee.clear(); }}>
              <Select.Trigger class="chip who" aria-label={`Assigned to ${owner?.name || "nobody"}`}>
                {@render magnet(owner, true)}<span>{owner?.name || "Nobody"}</span>
              </Select.Trigger>
              <Select.Portal>
                <Select.Content class="menu" sideOffset={6}>
                  <Select.Viewport>
                    <Select.Item value="" label="Nobody" class="menu-item"><span>Nobody</span></Select.Item>
                    {#each doc.current.people as p (p.$id)}
                      <Select.Item value={p.$id} label={p.name || "Unnamed"} class="menu-item">
                        {#snippet children({ selected })}{@render magnet(p, true)}<span>{p.name || "Unnamed"}</span>{#if selected}<Check size={14} />{/if}{/snippet}
                      </Select.Item>
                    {/each}
                  </Select.Viewport>
                </Select.Content>
              </Select.Portal>
            </Select.Root>
            <Select.Root type="single" value={chore.day} items={dayItems} onValueChange={(value) => { if (isDay(value)) row.day.set(value); }}>
              <Select.Trigger class="chip" aria-label={`Day: ${chore.day}`}>{dayItems.find((d) => d.value === chore.day)?.label}</Select.Trigger>
              <Select.Portal>
                <Select.Content class="menu" sideOffset={6}>
                  <Select.Viewport>
                    {#each dayItems as item (item.value)}
                      <Select.Item value={item.value} label={item.label} class="menu-item">
                        {#snippet children({ selected })}<span>{item.label}</span>{#if selected}<Check size={14} />{/if}{/snippet}
                      </Select.Item>
                    {/each}
                  </Select.Viewport>
                </Select.Content>
              </Select.Portal>
            </Select.Root>
            <button class="chip pts" aria-label={`Points: ${chore.points}. Change`} onclick={() => row.points.set(chore.points % 5 + 1)}>{chore.points} pt{chore.points === 1 ? "" : "s"}</button>
            <button class="chip rot" aria-pressed={chore.rotates} aria-label={chore.rotates ? "Rotates each week" : "Stays with the same person"} onclick={() => row.rotates.set(!chore.rotates)}><Repeat size={13} />{chore.rotates ? "Rotates" : "Fixed"}</button>
          </div>
        </li>
      {:else}
        <li class="empty"><h3>No chores yet.</h3><p>Add the first one below.</p></li>
      {/each}
    </ul>
    <form class="composer" onsubmit={(event) => { event.preventDefault(); addChore(); }}>
      <input bind:value={choreDraft} aria-label="New chore" placeholder="Add a chore…" />
      <Button.Root class="add" type="submit" aria-label="Add chore" disabled={!choreDraft.trim()}><Plus size={18} /></Button.Root>
    </form>
  </section>

  <aside class="rules">
    <span class="pin" aria-hidden="true"></span>
    <textarea aria-label="House rules" placeholder="House rules…" use:bindText={doc.fields.rules}></textarea>
  </aside>
</main>
