<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { bindText, draft, getDocument } from "hitslop/svelte";
  import type { Value } from "hitslop";
  import { Checkbox } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import Check from "@lucide/svelte/icons/check";
  import schema from "./schema";
  import { addPoint, choicePointView, removePoint, setNextDone, type ListKey } from "./choices";

  const doc = getDocument(schema);
  type ChoiceItem = Value<typeof schema.descriptor>["hooks"][number];
  const view = $derived(choicePointView(doc.current));

  const composers = {
    hooks: draft(text => doc.change(tx => addPoint(tx, "hooks", text))),
    awayMoves: draft(text => doc.change(tx => addPoint(tx, "awayMoves", text))),
    towardsMoves: draft(text => doc.change(tx => addPoint(tx, "towardsMoves", text))),
    helpers: draft(text => doc.change(tx => addPoint(tx, "helpers", text))),
  };

  function autosize(node: HTMLTextAreaElement) {
    const resize = () => {
      node.style.height = "0px";
      node.style.height = `${node.scrollHeight}px`;
    };
    resize();
    node.addEventListener("input", resize);
    return { update: resize, destroy: () => node.removeEventListener("input", resize) };
  }
</script>

{#snippet addForm(key: ListKey, label: string)}
  <form class="cp-add" data-slop-export="hide" onsubmit={(event) => composers[key].submit(event)}>
    <input bind:value={composers[key].value} placeholder={`Add ${label}…`} aria-label={`New ${label}`} />
    <button type="submit" aria-label={`Add ${label}`} disabled={!composers[key].ready}><Plus size={14} /></button>
  </form>
{/snippet}

{#snippet moves(key: ListKey, items: readonly ChoiceItem[], label: string)}
  <ul class="cp-moves">
    {#each items as item, index (item.$id)}
      <li>
        <textarea rows={1} use:bindText={doc.at(item).text} use:autosize aria-label={`${label} ${index + 1}`}></textarea>
        <button type="button" class="cp-remove" data-slop-export="hide" onclick={() => doc.change(tx => removePoint(tx, key, item.$id))} aria-label={`Remove ${label} ${index + 1}`}>
          <X size={12} />
        </button>
      </li>
    {/each}
  </ul>
  {@render addForm(key, label)}
{/snippet}

{#snippet chips(key: ListKey, items: readonly ChoiceItem[], label: string)}
  <div class="cp-chips">
    {#each items as item, index (item.$id)}
      <span class="cp-chip">
        <span class="cp-chip-text" data-value={item.text}>
          <input use:bindText={doc.at(item).text} size={1} aria-label={`${label} ${index + 1}`} />
        </span>
        <button type="button" class="cp-remove" data-slop-export="hide" onclick={() => doc.change(tx => removePoint(tx, key, item.$id))} aria-label={`Remove ${label} ${index + 1}`}>
          <X size={11} />
        </button>
      </span>
    {/each}
    {@render addForm(key, label)}
  </div>
{/snippet}

  <main class="cp-sheet">
    <header class="cp-head">
      <div>
        <h1>Choice Point</h1>
        <p>Notice what hooks you, then choose your next move.</p>
      </div>
      <input class="cp-date" use:bindText={doc.fields.date} aria-label="Date" />
    </header>

    <div class="cp-map">
      <section class="cp-card cp-base" aria-labelledby="cp-base-title">
        <h2 id="cp-base-title">Situation, thoughts & feelings</h2>
        <textarea class="cp-situation" rows={2} use:bindText={doc.fields.situation} use:autosize aria-label="Situation" placeholder="What’s happening? Describe it like a camera would."></textarea>
        <h3>What’s hooking me</h3>
        {@render chips("hooks", view.lists.hooks, "hook")}
      </section>

      <section class="cp-card cp-away" aria-labelledby="cp-away-title">
        <h2 id="cp-away-title">Away moves</h2>
        <p class="cp-hint">What I do when I’m hooked</p>
        {@render moves("awayMoves", view.lists.awayMoves, "away move")}
      </section>

      <section class="cp-card cp-towards" aria-labelledby="cp-towards-title">
        <h2 id="cp-towards-title">Towards moves</h2>
        <p class="cp-hint">What the person I want to be would do</p>
        {@render moves("towardsMoves", view.lists.towardsMoves, "towards move")}
        <h3>Helps me unhook</h3>
        {@render chips("helpers", view.lists.helpers, "value or skill")}
      </section>

      <svg class="cp-fork" viewBox="0 0 400 64" preserveAspectRatio="none" aria-hidden="true">
        <path class="cp-fork-stem" d="M200 64 V40" />
        <path class="cp-fork-away" d="M200 40 C170 24 120 14 100 4" />
        <path class="cp-fork-towards" d="M200 40 C230 24 280 14 300 4" />
      </svg>
      <span class="cp-point" aria-hidden="true">choice point</span>
    </div>

    <section class="cp-card cp-next" aria-labelledby="cp-next-title">
      <h2 id="cp-next-title">One towards move</h2>
      <textarea rows={1} use:bindText={doc.fields.nextMove} use:autosize aria-label="One towards move" placeholder="Something small I can do soon"></textarea>
      <div class="cp-next-row">
        <label>When <input use:bindText={doc.fields.nextWhen} placeholder="A real time" /></label>
        <label class="cp-done">
          <Checkbox.Root
            class="cp-check"
            aria-label="Done"
            checked={doc.current.nextDone}
            onCheckedChange={(checked) => doc.change(tx => setNextDone(tx, checked === true))}
          >
            {#snippet children({ checked })}
              {#if checked}<Check size={12} strokeWidth={3} />{/if}
            {/snippet}
          </Checkbox.Root>
          Done
        </label>
      </div>
    </section>
  </main>
