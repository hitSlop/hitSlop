<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { bindText, getDocument } from "hitslop/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { Tabs, Select, Button, Checkbox } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import Check from "@lucide/svelte/icons/check";
  import schema, { stages } from "./schema";
  import { dateString, dueLabel } from "./dates";
  import {
    addApplication, addChat, removeApplication, removeChat, setFollowUp, setMetDate, setStage, setThanked,
    trackerView, type Stage,
  } from "./pipeline";

  const doc = getDocument(schema);
  const stageItems = stages.map((value) => ({ value, label: value }));
  const today = dateString(new Date());
  // The open tab is window state; the export is a separate page and shows both lists.
  let tab = $state<"applications" | "chats">("applications");
  let company = $state("");
  let role = $state("");
  let person = $state("");
  let place = $state("");

  const view = $derived(trackerView(doc.current, today));
  const flipMs = $derived(prefersReducedMotion.current ? 0 : 200);

  function isStage(value: string): value is Stage {
    return stages.some((stage) => stage === value);
  }

  async function addApplicationRow(event: SubmitEvent) {
    event.preventDefault();
    await doc.change((tx) => addApplication(tx, company, role));
    company = "";
    role = "";
  }

  async function addChatRow(event: SubmitEvent) {
    event.preventDefault();
    await doc.change((tx) => addChat(tx, person, place));
    person = "";
    place = "";
  }
</script>

<main class="badge-wrap" aria-label="Internship tracker">
  <div class="strap" aria-hidden="true"><span class="slot"></span></div>
  <article class="badge">
    <header class="band">
      <p>Hello, I’m applying to</p>
    </header>
    <div class="title-block">
      <input class="title" aria-label="Search title" placeholder="Summer internships" use:bindText={doc.fields.title} />
      <p class="nudge" data-hot={view.nudges > 0}>{view.nudgeLabel}</p>
    </div>

    <div class="tally" role="img" aria-label={stages.map((stage) => `${view.counts[stage]} ${stage}`).join(", ")}>
      {#each stages as stage}
        <span class="seg" data-stage={stage} style:flex-grow={view.counts[stage]} hidden={view.counts[stage] === 0}></span>
      {/each}
    </div>
    <ul class="legend" aria-hidden="true">
      {#each stages as stage}
        <li data-stage={stage}><b>{view.counts[stage]}</b> {stage}</li>
      {/each}
    </ul>

    <Tabs.Root value={tab} onValueChange={(value) => { if (value === "applications" || value === "chats") tab = value; }}>
      <Tabs.List class="tabs" aria-label="Sections">
        <Tabs.Trigger value="applications" class="tab">Applications <span>{view.applications.length}</span></Tabs.Trigger>
        <Tabs.Trigger value="chats" class="tab">Coffee chats <span>{view.chats.length}</span></Tabs.Trigger>
      </Tabs.List>

      <Tabs.Content value="applications">
        <form class="composer" onsubmit={addApplicationRow}>
          <input bind:value={company} aria-label="Company" placeholder="Company" />
          <input bind:value={role} aria-label="Role" placeholder="Role" />
          <Button.Root class="add" type="submit" aria-label="Add application" disabled={!company.trim()}><Plus size={18} /></Button.Root>
        </form>
        <ul class="rows">
          {#each view.applications as app (app.$id)}
            {@const row = doc.at(app)}
            {@const due = dueLabel(app.followUp, today)}
            <li class="row" data-stage={app.stage} animate:flip={{ duration: flipMs }}>
              <Select.Root type="single" value={app.stage} items={stageItems} onValueChange={(value) => { if (isStage(value)) doc.change((tx) => setStage(tx, app.$id, value)); }}>
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
                <input type="date" aria-label={`Follow-up date for ${app.company || "application"}`} value={app.followUp ?? ""} onchange={(event) => doc.change((tx) => setFollowUp(tx, app.$id, event.currentTarget.value))} />
                {#if due && app.stage !== "offer" && app.stage !== "rejected"}<span>{due.text}</span>{/if}
              </div>
              <button class="remove" aria-label={`Remove ${app.company || "application"}`} onclick={() => doc.change((tx) => removeApplication(tx, app.$id))}><X size={14} /></button>
            </li>
          {:else}
            <li class="empty"><h2>No applications yet.</h2><p>Add the first company above.</p></li>
          {/each}
        </ul>
      </Tabs.Content>

      <Tabs.Content value="chats">
        <form class="composer" onsubmit={addChatRow}>
          <input bind:value={person} aria-label="Person" placeholder="Who" />
          <input bind:value={place} aria-label="Where or company" placeholder="Company or place" />
          <Button.Root class="add" type="submit" aria-label="Add chat" disabled={!person.trim()}><Plus size={18} /></Button.Root>
        </form>
        <ul class="rows">
          {#each view.chats as chat (chat.$id)}
            {@const row = doc.at(chat)}
            <li class="row chat" animate:flip={{ duration: flipMs }}>
              <Checkbox.Root class="thanks" checked={chat.thanked} onCheckedChange={(checked) => doc.change((tx) => setThanked(tx, chat.$id, checked === true))} aria-label={`Thank-you sent to ${chat.name || "contact"}`}>
                {#snippet children({ checked })}{#if checked}<Check size={14} strokeWidth={3} />{/if}{/snippet}
              </Checkbox.Root>
              <div class="who">
                <input class="company" aria-label="Name" placeholder="Name" use:bindText={row.name} />
                <input class="role" aria-label="Company or place" placeholder="Company or place" use:bindText={row.place} />
              </div>
              <div class="follow">
                <input type="date" aria-label={`Date met ${chat.name || "contact"}`} value={chat.when ?? ""} onchange={(event) => doc.change((tx) => setMetDate(tx, chat.$id, event.currentTarget.value))} />
                <span>{chat.thanked ? "thanked" : "thank-you due"}</span>
              </div>
              <button class="remove" aria-label={`Remove ${chat.name || "chat"}`} onclick={() => doc.change((tx) => removeChat(tx, chat.$id))}><X size={14} /></button>
            </li>
          {:else}
            <li class="empty"><h2>No coffee chats yet.</h2><p>Log the people who helped, and thank them.</p></li>
          {/each}
        </ul>
      </Tabs.Content>
    </Tabs.Root>
  </article>
</main>
