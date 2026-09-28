<script lang="ts">
  import { Slop, bindText, bindValue, useDocument } from "@hitslop/document/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { Tabs, Toggle, Select, Button, Tooltip } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Star from "@lucide/svelte/icons/star";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import List from "@lucide/svelte/icons/list";
  import Calendar from "@lucide/svelte/icons/calendar";
  import schema, { type JournalEntry } from "./schema";
  import { SIGNIFIERS, signifierItems, isSignifier, symbolFor, labelFor, nextSignifier, type Signifier } from "./signifiers";

  type Spread = "daily" | "monthly";

  const doc = useDocument(schema);
  let spread = $state<Spread>("daily");
  let draft = $state("");
  let draftType = $state<Signifier>("task");
  let draftStar = $state(false);
  let composer = $state<HTMLInputElement>();

  const openCount = $derived(doc.current.entries.filter((item) => item.type === "task" || item.type === "scheduled").length);
  const flipMs = $derived(prefersReducedMotion.current ? 0 : 220);

  function isSpread(value: string): value is Spread {
    return value === "daily" || value === "monthly";
  }
  function addEntry() {
    const text = draft.trim();
    if (!text) return;
    doc.fields.entries.insert({ type: draftType, star: draftStar, text });
    draft = "";
    draftStar = false;
    composer?.focus();
  }
  function cycleSignifier(item: JournalEntry) {
    doc.at(item).type.set(nextSignifier(item.type));
  }
  function addMonthDay() {
    const last = doc.current.monthlyLog.at(-1);
    const day = Math.min(31, (last?.day ?? 0) + 1);
    doc.fields.monthlyLog.insert({ day, weekday: "", text: "" });
  }
</script>

<Slop>
  <Tooltip.Provider>
    <main class="page" data-slop-selection="none" aria-label="Bullet journal">
      <span class="ribbon" aria-hidden="true"></span>

      <header class="header">
        <Tabs.Root value={spread} onValueChange={(value) => { if (isSpread(value)) spread = value; }}>
          <Tabs.List class="tabs" data-slop-export="hide" aria-label="Journal spread">
            <Tabs.Trigger value="daily" class="tab"><List size={12} strokeWidth={2.2} /> Daily Rapid Log</Tabs.Trigger>
            <Tabs.Trigger value="monthly" class="tab"><Calendar size={12} strokeWidth={2.2} /> Monthly Index</Tabs.Trigger>
          </Tabs.List>
        </Tabs.Root>
        <div class="legend" data-slop-export="hide">
          {#each SIGNIFIERS as item}
            <span class="legendItem" title={item.label}><strong class="legendSym">{item.symbol}</strong>{item.label}</span>
          {/each}
          <span class="legendItem" title="Priority"><strong class="legendSym">*</strong>Priority</span>
        </div>
      </header>

      <article class="spread" aria-label={spread === "daily" ? "Daily rapid log" : "Monthly index"}>
        {#if spread === "daily"}
          <div class="titleRow">
            <input class="title" aria-label="Log date" use:bindText={doc.fields.date} placeholder="Today" />
            <label class="pageNumWrap"><span class="srOnly">Daily page number</span><span aria-hidden="true">pg.</span> <input class="pageNum" use:bindText={doc.fields.dailyPage} /></label>
          </div>

          <ul class="list">
            {#each doc.current.entries as item (item.$id)}
              <li class="row" data-complete={item.type === "complete"} animate:flip={{ duration: flipMs }}>
                <Toggle.Root
                  type="button"
                  pressed={item.star}
                  onPressedChange={(pressed) => doc.at(item).star.set(pressed)}
                  class="starBtn"
                  aria-label={item.star ? "Remove priority" : "Mark as priority"}
                >
                  {#if item.star}
                    <Star size={11} fill="currentColor" />
                  {:else}
                    <span aria-hidden="true">·</span>
                  {/if}
                </Toggle.Root>
                <Tooltip.Root>
                  <Tooltip.Trigger
                    class="signifier"
                    data-type={item.type}
                    type="button"
                    aria-label="{labelFor(item.type)} signifier. Cycle to the next mark."
                    onclick={() => cycleSignifier(item)}
                  >{symbolFor(item.type)}</Tooltip.Trigger>
                  <Tooltip.Portal>
                    <Tooltip.Content class="tooltip" sideOffset={6}>Click to cycle • × › ‹ ○ —</Tooltip.Content>
                  </Tooltip.Portal>
                </Tooltip.Root>
                <input class="entryText" aria-label="Rapid log entry" use:bindText={doc.at(item).text} placeholder="Task, event, or note" />
                <Tooltip.Root>
                  <Tooltip.Trigger class="remove" data-slop-export="hide" type="button" aria-label="Delete {item.text || 'entry'}" onclick={() => doc.fields.entries.remove(item.$id)}>
                    <Trash2 size={12} />
                  </Tooltip.Trigger>
                  <Tooltip.Portal>
                    <Tooltip.Content class="tooltip" sideOffset={6}>Remove this line</Tooltip.Content>
                  </Tooltip.Portal>
                </Tooltip.Root>
              </li>
            {:else}
              <li class="empty">
                <h2>A blank page.</h2>
                <p>Rapid log a task, event, or note. Click the bullet to cycle its signifier.</p>
              </li>
            {/each}
          </ul>

          <form class="composer" data-slop-export="hide" onsubmit={(event) => { event.preventDefault(); addEntry(); }}>
            <Toggle.Root
              type="button"
              pressed={draftStar}
              onPressedChange={(pressed) => { draftStar = pressed; }}
              class="starToggle"
              aria-label="Mark new entry as priority"
            >
              <Star size={12} fill={draftStar ? "currentColor" : "none"} />
            </Toggle.Root>
            <Select.Root type="single" value={draftType} items={signifierItems} onValueChange={(value) => { if (isSignifier(value)) draftType = value; }}>
              <Select.Trigger class="selectTrigger" type="button" aria-label="Signifier for new entry">
                <Select.Value placeholder="Signifier" />
                <ChevronDown size={12} strokeWidth={2.2} />
              </Select.Trigger>
              <Select.Portal>
                <Select.Content class="selectContent" sideOffset={6}>
                  <Select.Viewport>
                    {#each signifierItems as item (item.value)}
                      <Select.Item value={item.value} label={item.label}>
                        {#snippet children({ selected })}
                          {item.label}{#if selected}<Check size={12} strokeWidth={2.4} />{/if}
                        {/snippet}
                      </Select.Item>
                    {/each}
                  </Select.Viewport>
                </Select.Content>
              </Select.Portal>
            </Select.Root>
            <input bind:this={composer} class="addInput" aria-label="New rapid log entry" placeholder="Rapid log an item…" bind:value={draft} />
            <Button.Root class="add" type="submit" aria-label="Add entry" disabled={!draft.trim()}><Plus size={14} /></Button.Root>
          </form>
        {:else}
          <div class="titleRow">
            <input class="title" aria-label="Month title" use:bindText={doc.fields.monthTitle} placeholder="Month" />
            <label class="pageNumWrap"><span class="srOnly">Monthly page number</span><span aria-hidden="true">pg.</span> <input class="pageNum" use:bindText={doc.fields.monthlyPage} /></label>
          </div>

          <ul class="list">
            {#each doc.current.monthlyLog as item (item.$id)}
              <li class="monthRow" animate:flip={{ duration: flipMs }}>
                <input class="monthDay" type="number" min="1" max="31" aria-label="Day of month" use:bindValue={doc.at(item).day} />
                <input class="monthWeekday" aria-label="Weekday" maxlength="3" placeholder="—" use:bindText={doc.at(item).weekday} />
                <input class="monthText" aria-label="Monthly event for day {item.day}" placeholder="Key event or milestone…" use:bindText={doc.at(item).text} />
                <button class="remove" data-slop-export="hide" type="button" aria-label="Delete day {item.day}" onclick={() => doc.fields.monthlyLog.remove(item.$id)}><Trash2 size={12} /></button>
              </li>
            {:else}
              <li class="empty">
                <h2>No days indexed.</h2>
                <p>Add the dates that matter this month.</p>
              </li>
            {/each}
          </ul>
          <div class="monthComposer" data-slop-export="hide">
            <Button.Root class="addDay" type="button" onclick={addMonthDay}>+ Add day</Button.Root>
          </div>
        {/if}
      </article>

      <footer class="foot">
        {#if spread === "daily"}
          <span>{doc.current.entries.length} bullets · {openCount} open</span>
          <span>Rapid log</span>
        {:else}
          <span>{doc.current.monthlyLog.length} days indexed</span>
          <span>Monthly log</span>
        {/if}
      </footer>
    </main>
  </Tooltip.Provider>

  {#snippet exportView()}
    <article class="exportPage" aria-label={spread === "daily" ? "Exported daily rapid log" : "Exported monthly index"}>
      <span class="ribbon" aria-hidden="true"></span>
      {#if spread === "daily"}
        <div class="titleRow">
          <h1 class="title">{doc.current.date.trim() || "Rapid log"}</h1>
          <span class="pageNumWrap">pg. {doc.current.dailyPage.trim() || "—"}</span>
        </div>
        <ul class="list">
          {#each doc.current.entries as item (item.$id)}
            <li class="row" data-complete={item.type === "complete"}>
              <span class="exportStar" aria-hidden="true">{item.star ? "*" : ""}</span>
              <span class="exportMark" data-type={item.type}>{symbolFor(item.type)}</span>
              <span class="exportText">{item.text.trim() || "Untitled"}</span>
            </li>
          {:else}
            <li class="empty"><h2>A blank page.</h2></li>
          {/each}
        </ul>
        <footer class="foot">
          <span>{doc.current.entries.length} bullets · {openCount} open</span>
          <span>Rapid log</span>
        </footer>
      {:else}
        <div class="titleRow">
          <h1 class="title">{doc.current.monthTitle.trim() || "Monthly index"}</h1>
          <span class="pageNumWrap">pg. {doc.current.monthlyPage.trim() || "—"}</span>
        </div>
        <ul class="list">
          {#each doc.current.monthlyLog as item (item.$id)}
            <li class="monthRow">
              <span class="monthDay">{String(item.day).padStart(2, "0")}</span>
              <span class="monthWeekday">{item.weekday.trim() || "—"}</span>
              <span class="monthText">{item.text.trim() || ""}</span>
            </li>
          {:else}
            <li class="empty"><h2>No days indexed.</h2></li>
          {/each}
        </ul>
        <footer class="foot">
          <span>{doc.current.monthlyLog.length} days indexed</span>
          <span>Monthly log</span>
        </footer>
      {/if}
    </article>
  {/snippet}

  {#snippet icon()}
    <div class="iconSurface" aria-hidden="true">
      <div class="iconPlate">
        <span class="iconRibbon"></span>
        {#each ["•", "×", "›", "○"] as mark}
          <div class="iconRow">
            <span class="iconBullet">{mark}</span>
            <i class="iconLine"></i>
          </div>
        {/each}
      </div>
    </div>
  {/snippet}
</Slop>
