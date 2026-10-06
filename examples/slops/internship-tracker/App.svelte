<script lang="ts">
  import { bindText } from "@hitslop/document/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { Tabs, Select, Button, Checkbox } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import Check from "@lucide/svelte/icons/check";
  import doc, { stages, type Application } from "./schema";
  import { dueLabel } from "./dates";
  import { stageCounts, needsNudge } from "./pipeline";
  import { ui } from "./ui.svelte";

  const stageItems = stages.map((value) => ({ value, label: value }));
  let company = $state("");
  let role = $state("");
  let person = $state("");
  let place = $state("");

  const counts = $derived(stageCounts(doc.current.applications));
  const nudges = $derived(doc.current.applications.filter(needsNudge).length);
  const flipMs = $derived(prefersReducedMotion.current ? 0 : 200);

  function isStage(value: string): value is (typeof stages)[number] {
    return stages.some((stage) => stage === value);
  }

  function addApplication() {
    const name = company.trim();
    if (!name) return;
    doc.fields.applications.insert({ company: name, role: role.trim(), stage: "applied" });
    company = "";
    role = "";
  }

  function addChat() {
    const name = person.trim();
    if (!name) return;
    doc.fields.chats.insert({ name, place: place.trim(), thanked: false });
    person = "";
    place = "";
  }

  function setDate(row: Application, value: string) {
    const handle = doc.at(row).followUp;
    if (value) handle.set(value);
    else handle.clear();
  }
</script>

<main class="badge-wrap" data-slop-selection="none" aria-label="Internship tracker">
  <div class="strap" aria-hidden="true"><span class="slot"></span></div>
  <article class="badge">
    <header class="band">
      <p>Hello, I’m applying to</p>
    </header>
    <div class="title-block">
      <input class="title" aria-label="Search title" placeholder="Summer internships" use:bindText={doc.fields.title} />
      <p class="nudge" data-hot={nudges > 0}>
        {nudges > 0 ? `${nudges} follow-up${nudges === 1 ? "" : "s"} due` : "No follow-ups due"}
      </p>
    </div>

    <div class="tally" role="img" aria-label={stages.map((stage) => `${counts[stage]} ${stage}`).join(", ")}>
      {#each stages as stage}
        <span class="seg" data-stage={stage} style:flex-grow={counts[stage]} hidden={counts[stage] === 0}></span>
      {/each}
    </div>
    <ul class="legend" aria-hidden="true">
      {#each stages as stage}
        <li data-stage={stage}><b>{counts[stage]}</b> {stage}</li>
      {/each}
    </ul>

    <Tabs.Root value={ui.tab} onValueChange={(value) => { if (value === "applications" || value === "chats") ui.tab = value; }}>
      <Tabs.List class="tabs" aria-label="Sections">
        <Tabs.Trigger value="applications" class="tab">Applications <span>{doc.current.applications.length}</span></Tabs.Trigger>
        <Tabs.Trigger value="chats" class="tab">Coffee chats <span>{doc.current.chats.length}</span></Tabs.Trigger>
      </Tabs.List>

      <Tabs.Content value="applications">
        <form class="composer" onsubmit={(event) => { event.preventDefault(); addApplication(); }}>
          <input bind:value={company} aria-label="Company" placeholder="Company" />
          <input bind:value={role} aria-label="Role" placeholder="Role" />
          <Button.Root class="add" type="submit" aria-label="Add application" disabled={!company.trim()}><Plus size={18} /></Button.Root>
        </form>
        <ul class="rows">
          {#each doc.current.applications as app (app.$id)}
            {@const row = doc.at(app)}
            {@const due = dueLabel(app.followUp)}
            <li class="row" data-stage={app.stage} animate:flip={{ duration: flipMs }}>
              <Select.Root type="single" value={app.stage} items={stageItems} onValueChange={(value) => { if (isStage(value)) row.stage.set(value); }}>
                <Select.Trigger class="stamp" aria-label={`Stage for ${app.company || "application"}: ${app.stage}`}>{app.stage}</Select.Trigger>
                <Select.Portal>
                  <Select.Content class="menu" sideOffset={6}>
                    <Select.Viewport>
                      {#each stageItems as item (item.value)}
                        <Select.Item value={item.value} label={item.label} class="menu-item" data-stage={item.value}>
                          {#snippet children({ selected })}<span>{item.label}</span>{#if selected}<Check size={14} />{/if}{/snippet}
                        </Select.Item>
                      {/each}
                    </Select.Viewport>
                  </Select.Content>
                </Select.Portal>
              </Select.Root>
              <div class="who">
                <input class="company" aria-label="Company" placeholder="Company" use:bindText={row.company} />
                <input class="role" aria-label="Role" placeholder="Role" use:bindText={row.role} />
              </div>
              <div class="follow" data-late={due?.late ? "" : undefined}>
                <input type="date" aria-label={`Follow-up date for ${app.company || "application"}`} value={app.followUp ?? ""} onchange={(event) => setDate(app, event.currentTarget.value)} />
                {#if due && app.stage !== "offer" && app.stage !== "rejected"}<span>{due.text}</span>{/if}
              </div>
              <button class="remove" aria-label={`Remove ${app.company || "application"}`} onclick={() => doc.fields.applications.remove(app.$id)}><X size={14} /></button>
            </li>
          {:else}
            <li class="empty"><h2>No applications yet.</h2><p>Add the first company above.</p></li>
          {/each}
        </ul>
      </Tabs.Content>

      <Tabs.Content value="chats">
        <form class="composer" onsubmit={(event) => { event.preventDefault(); addChat(); }}>
          <input bind:value={person} aria-label="Person" placeholder="Who" />
          <input bind:value={place} aria-label="Where or company" placeholder="Company or place" />
          <Button.Root class="add" type="submit" aria-label="Add chat" disabled={!person.trim()}><Plus size={18} /></Button.Root>
        </form>
        <ul class="rows">
          {#each doc.current.chats as chat (chat.$id)}
            {@const row = doc.at(chat)}
            <li class="row chat" animate:flip={{ duration: flipMs }}>
              <Checkbox.Root class="thanks" checked={chat.thanked} onCheckedChange={(checked) => row.thanked.set(checked === true)} aria-label={`Thank-you sent to ${chat.name || "contact"}`}>
                {#snippet children({ checked })}{#if checked}<Check size={14} strokeWidth={3} />{/if}{/snippet}
              </Checkbox.Root>
              <div class="who">
                <input class="company" aria-label="Name" placeholder="Name" use:bindText={row.name} />
                <input class="role" aria-label="Company or place" placeholder="Company or place" use:bindText={row.place} />
              </div>
              <div class="follow">
                <input type="date" aria-label={`Date met ${chat.name || "contact"}`} value={chat.when ?? ""} onchange={(event) => { const v = event.currentTarget.value; if (v) row.when.set(v); else row.when.clear(); }} />
                <span>{chat.thanked ? "thanked" : "thank-you due"}</span>
              </div>
              <button class="remove" aria-label={`Remove ${chat.name || "chat"}`} onclick={() => doc.fields.chats.remove(chat.$id)}><X size={14} /></button>
            </li>
          {:else}
            <li class="empty"><h2>No coffee chats yet.</h2><p>Log the people who helped, and thank them.</p></li>
          {/each}
        </ul>
      </Tabs.Content>
    </Tabs.Root>
  </article>
</main>
