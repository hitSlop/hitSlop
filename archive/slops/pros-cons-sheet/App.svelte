<script lang="ts">
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import { onDestroy, untrack } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { Slider, Select, Button } from "bits-ui";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Check from "@lucide/svelte/icons/check";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import schema, { type Factor, type Side } from "./schema";
  import { beamTilt, clampWeight, totalWeight } from "./balance";
  import { STATUSES, isStatus } from "./statuses";
  import { grow } from "./grow";
  import Scale from "./Scale.svelte";

  const doc = useDocument(schema);

  const pros = $derived(doc.current.factors.filter((item) => item.side === "pro"));
  const cons = $derived(doc.current.factors.filter((item) => item.side === "con"));
  const proTotal = $derived(totalWeight(pros));
  const conTotal = $derived(totalWeight(cons));
  const tiltTarget = $derived(beamTilt(proTotal, conTotal));
  const tilt = new Tween(untrack(() => tiltTarget), { duration: 420, easing: cubicOut });
  let initialized = false;
  const statusLabel = $derived(STATUSES.find(item => item.value === doc.current.status)?.label ?? "Evaluating");

  $effect(() => {
    const instant = !initialized || prefersReducedMotion.current;
    void tilt.set(tiltTarget, { duration: instant ? 0 : 420, delay: 0 });
    initialized = true;
  });

  onDestroy(() => {
    void tilt.set(tilt.target, { duration: 0, delay: 0 });
  });

  let draftPro = $state("");
  let draftProWeight = $state(3);
  let draftCon = $state("");
  let draftConWeight = $state(3);
  let proComposer = $state<HTMLInputElement>();
  let conComposer = $state<HTMLInputElement>();

  function addFactor(side: Side) {
    const text = (side === "pro" ? draftPro : draftCon).trim();
    if (!text) return;
    const weight = clampWeight(side === "pro" ? draftProWeight : draftConWeight);
    doc.fields.factors.insert({ text, weight, side });
    if (side === "pro") {
      draftPro = "";
      draftProWeight = 3;
      proComposer?.focus();
    } else {
      draftCon = "";
      draftConWeight = 3;
      conComposer?.focus();
    }
  }

  function removeFactor(id: string) {
    doc.fields.factors.remove(id);
  }

  function setWeight(item: Factor, value: number) {
    const weight = clampWeight(value);
    if (weight === item.weight) return;
    doc.at(item).weight.set(weight);
  }
</script>

<Slop>
<main class="desk" data-slop-selection="none" aria-label="Decision balance">
  <article class="letter">
    <header class="letterhead">
      <div class="metaRow">
        <h1 class="stamp">Decision Balance</h1>
        <label>
          <span class="srOnly">Decision date</span>
          <input class="dateField" use:bindText={doc.fields.date} />
        </label>
      </div>
      <div class="questionRow">
        <label class="whether" for="decision-question">Whether to…</label>
        <textarea id="decision-question" class="question" aria-label="Decision statement" placeholder="What are you deciding?" rows={1} use:bindText={doc.fields.question} use:grow={doc.current.question}></textarea>
      </div>
      <Scale tilt={tilt.current} {proTotal} {conTotal} />
    </header>

    <div class="spread">
      <section class="column" data-side="for" aria-labelledby="for-heading">
        <div class="columnHead">
          <div>
            <h2 id="for-heading" class="columnTitle">Pros</h2>
            <span class="columnHint">Reasons in favor</span>
          </div>
          <span class="columnTotal">{proTotal} pts</span>
        </div>
        <ul class="list">
          {#each pros as item (item.$id)}
            <li class="row">
              <textarea class="reason" aria-label="Reason for" placeholder="A reason in favor" rows={1} use:bindText={doc.at(item).text} use:grow={item.text}></textarea>
              <div class="weight">
                <span class="weightLabel">Importance <strong>{clampWeight(item.weight)}/5</strong></span>
                <Slider.Root type="single" value={clampWeight(item.weight)} onValueChange={value => setWeight(item, value)} min={1} max={5} step={1} class="slider" aria-label="Importance for {item.text || 'this reason'}, 1 minor to 5 decisive">
                  {#snippet children({ thumbs })}
                    <span class="track"><Slider.Range class="range" /></span>
                    {#each thumbs as index}<Slider.Thumb {index} class="thumb" aria-label="Importance for {item.text || 'this reason'}, 1 minor to 5 decisive" />{/each}
                  {/snippet}
                </Slider.Root>
                <Button.Root type="button" class="remove" data-slop-export="hide" aria-label="Remove reason for: {item.text || 'untitled'}" onclick={() => removeFactor(item.$id)}><Trash2 size={12} strokeWidth={1.8} /></Button.Root>
              </div>
            </li>
          {:else}
            <li class="empty">No motives yet. Add a reason in favor.</li>
          {/each}
        </ul>
        <form class="composer" data-slop-export="hide" onsubmit={event => { event.preventDefault(); addFactor("pro"); }}>
          <input bind:this={proComposer} class="addInput" aria-label="New reason for" placeholder="Add a reason in favor…" bind:value={draftPro} />
          <div class="weight"><span class="weightLabel">Importance <strong>{draftProWeight}/5</strong></span><Slider.Root type="single" value={draftProWeight} onValueChange={value => { draftProWeight = clampWeight(value); }} min={1} max={5} step={1} class="slider" aria-label="Importance for new reason in favor">
            {#snippet children({ thumbs })}
              <span class="track"><Slider.Range class="range" /></span>
              {#each thumbs as index}<Slider.Thumb {index} class="thumb" aria-label="Importance for new reason in favor" />{/each}
            {/snippet}
          </Slider.Root></div>
          <Button.Root class="add" type="submit" aria-label="Add reason for" disabled={!draftPro.trim()}><Plus size={14} strokeWidth={1.8} /></Button.Root>
        </form>
      </section>

      <section class="column" data-side="against" aria-labelledby="against-heading">
        <div class="columnHead">
          <div>
            <h2 id="against-heading" class="columnTitle">Cons</h2>
            <span class="columnHint">Reasons against</span>
          </div>
          <span class="columnTotal">{conTotal} pts</span>
        </div>
        <ul class="list">
          {#each cons as item (item.$id)}
            <li class="row">
              <textarea class="reason" aria-label="Reason against" placeholder="A concern" rows={1} use:bindText={doc.at(item).text} use:grow={item.text}></textarea>
              <div class="weight">
                <span class="weightLabel">Importance <strong>{clampWeight(item.weight)}/5</strong></span>
                <Slider.Root type="single" value={clampWeight(item.weight)} onValueChange={value => setWeight(item, value)} min={1} max={5} step={1} class="slider" aria-label="Importance against {item.text || 'this reason'}, 1 minor to 5 decisive">
                  {#snippet children({ thumbs })}
                    <span class="track"><Slider.Range class="range" /></span>
                    {#each thumbs as index}<Slider.Thumb {index} class="thumb" aria-label="Importance against {item.text || 'this reason'}, 1 minor to 5 decisive" />{/each}
                  {/snippet}
                </Slider.Root>
                <Button.Root type="button" class="remove" data-slop-export="hide" aria-label="Remove reason against: {item.text || 'untitled'}" onclick={() => removeFactor(item.$id)}><Trash2 size={12} strokeWidth={1.8} /></Button.Root>
              </div>
            </li>
          {:else}
            <li class="empty">No objections yet. Add a reason against.</li>
          {/each}
        </ul>
        <form class="composer" data-slop-export="hide" onsubmit={event => { event.preventDefault(); addFactor("con"); }}>
          <input bind:this={conComposer} class="addInput" aria-label="New reason against" placeholder="Add a reason against…" bind:value={draftCon} />
          <div class="weight"><span class="weightLabel">Importance <strong>{draftConWeight}/5</strong></span><Slider.Root type="single" value={draftConWeight} onValueChange={value => { draftConWeight = clampWeight(value); }} min={1} max={5} step={1} class="slider" aria-label="Importance for new reason against">
            {#snippet children({ thumbs })}
              <span class="track"><Slider.Range class="range" /></span>
              {#each thumbs as index}<Slider.Thumb {index} class="thumb" aria-label="Importance for new reason against" />{/each}
            {/snippet}
          </Slider.Root></div>
          <Button.Root class="add" type="submit" aria-label="Add reason against" disabled={!draftCon.trim()}><Plus size={14} strokeWidth={1.8} /></Button.Root>
        </form>
      </section>
    </div>

    <footer class="foot">
      <Select.Root type="single" value={doc.current.status} items={[...STATUSES]} onValueChange={value => { if (isStatus(value)) doc.fields.status.set(value); }}>
        <Select.Trigger class="seal" aria-label="Verdict status: {statusLabel}">
          <span class="sealFace"><Select.Value placeholder="Evaluating" /></span><ChevronDown size={15} />
        </Select.Trigger>
        <Select.Portal>
          <Select.Content class="selectContent" sideOffset={8}>
            <Select.Viewport>
              {#each STATUSES as item (item.value)}
                <Select.Item class="selectItem" value={item.value} label={item.label}>
                  {#snippet children({ selected })}{item.label}{#if selected}<Check size={12} strokeWidth={2.2} />{/if}{/snippet}
                </Select.Item>
              {/each}
            </Select.Viewport>
          </Select.Content>
        </Select.Portal>
      </Select.Root>
      <label class="verdict">
        <span class="verdictLabel">Verdict</span>
        <textarea class="verdictText" aria-label="Verdict notes" placeholder="Record the conclusion, terms, or next step…" use:bindText={doc.fields.verdict} use:grow={doc.current.verdict}></textarea>
      </label>
    </footer>
  </article>
</main>

{#snippet exportView()}
  <article class="exportLetter" aria-label="Exported decision balance">
    <header class="letterhead">
      <div class="metaRow">
        <span class="stamp">Decision Balance</span>
        <span class="dateField">{doc.current.date}</span>
      </div>
      <div class="questionRow">
        <span class="whether">Whether to</span>
        <h1 class="question">{doc.current.question.trim() || "state the choice clearly…"}</h1>
      </div>
      <Scale tilt={tiltTarget} {proTotal} {conTotal} />
    </header>

    <div class="spread">
      <section class="column" data-side="for" aria-labelledby="export-for">
        <div class="columnHead">
          <div>
            <h2 id="export-for" class="columnTitle">Pros</h2>
            <span class="columnHint">Reasons in favor</span>
          </div>
          <span class="columnTotal">{proTotal}</span>
        </div>
        <ul class="list">
          {#each pros as item (item.$id)}
            <li class="row">
              <span class="reason">{item.text.trim() || "Untitled reason"}</span>
              <span class="weightNum">{clampWeight(item.weight)}</span>
            </li>
          {:else}
            <li class="empty">No motives recorded.</li>
          {/each}
        </ul>
      </section>
      <section class="column" data-side="against" aria-labelledby="export-against">
        <div class="columnHead">
          <div>
            <h2 id="export-against" class="columnTitle">Cons</h2>
            <span class="columnHint">Reasons against</span>
          </div>
          <span class="columnTotal">{conTotal}</span>
        </div>
        <ul class="list">
          {#each cons as item (item.$id)}
            <li class="row">
              <span class="reason">{item.text.trim() || "Untitled reason"}</span>
              <span class="weightNum">{clampWeight(item.weight)}</span>
            </li>
          {:else}
            <li class="empty">No objections recorded.</li>
          {/each}
        </ul>
      </section>
    </div>

    <footer class="foot">
      <div class="seal" aria-label="Verdict status {statusLabel}">
        <span class="sealFace">{statusLabel}</span>
      </div>
      <div class="verdict">
        <span class="verdictLabel">Verdict</span>
        {#if doc.current.verdict.trim()}
          <p class="verdictExport">{doc.current.verdict}</p>
        {:else}
          <p class="empty">No conclusion recorded.</p>
        {/if}
      </div>
    </footer>
  </article>
{/snippet}

{#snippet icon()}
  <div class="iconSurface" aria-hidden="true">
    <div class="iconTile">
      <article class="iconSheet">
        <div class="iconHead">
          <div class="iconBeam" style:transform={`rotate(${tiltTarget}deg)`}>
            <span class="iconBar"></span>
            <span class="iconPan" style:transform={`rotate(${-tiltTarget}deg)`}></span>
            <span class="iconPan" style:transform={`rotate(${-tiltTarget}deg)`}></span>
          </div>
        </div>
        <div class="iconCols">
          <div class="iconCol">
            <span class="iconLine" data-side="for"></span>
            <span class="iconLine" data-side="for"></span>
            <span class="iconLine" data-side="for"></span>
          </div>
          <div class="iconCol">
            <span class="iconLine" data-side="against"></span>
            <span class="iconLine" data-side="against"></span>
          </div>
        </div>
      </article>
    </div>
  </div>
{/snippet}
</Slop>
