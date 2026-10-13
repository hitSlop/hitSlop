<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { bindText, getDocument } from "hitslop/svelte";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Play from "@lucide/svelte/icons/play";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import { Dialog } from "bits-ui";
  import { onDestroy } from "svelte";
  import schema, { layouts } from "./schema";
  import { addSlide, deckView, moveSlide, nextTheme, removeSlide, setLayout, type Slide, type SlideLayout } from "./deck";

  const doc = getDocument(schema);

  let isPresenting = $state(false);
  let timerSeconds = $state(0);
  let timerInterval: ReturnType<typeof setInterval> | undefined;

  // The selected slide is view state: it is not saved, and a capture shows the first slide.
  let selected = $state(0);
  const view = $derived(deckView(doc.current, selected));
  const currentSlide = $derived(view.current);

  function selectSlide(index: number) {
    selected = Math.max(0, Math.min(view.last, index));
  }

  async function add() {
    await doc.change((tx) => addSlide(tx));
    selected = doc.current.slides.length - 1;
  }

  async function remove(index: number) {
    const slide = doc.current.slides[index];
    if (!slide || doc.current.slides.length <= 1) return;
    const next = Math.min(view.index, doc.current.slides.length - 2);
    await doc.change((tx) => removeSlide(tx, doc.current, slide.$id));
    selected = next;
  }

  async function move(index: number, direction: -1 | 1) {
    const slide = doc.current.slides[index];
    if (!slide || !doc.current.slides[index + direction]) return;
    const active = view.index;
    await doc.change((tx) => moveSlide(tx, doc.current, slide.$id, direction));
    if (active === index) selected = index + direction;
    else if (active === index + direction) selected = index;
  }

  function changeLayout(layout: SlideLayout) {
    const slide = currentSlide;
    if (!slide || slide.layout === layout) return;
    return doc.change((tx) => setLayout(tx, doc.current, slide.$id, layout));
  }

  function cycleTheme() {
    return doc.fields.theme.set(nextTheme(doc.current.theme));
  }

  function startPresenting() {
    isPresenting = true;
    timerSeconds = 0;
    if (timerInterval) clearInterval(timerInterval);
    timerInterval = setInterval(() => { timerSeconds += 1; }, 1000);
  }

  function stopPresenting() {
    isPresenting = false;
    if (timerInterval) clearInterval(timerInterval);
    timerInterval = undefined;
  }

  function nextSlide() {
    selectSlide(view.index + 1);
  }

  function prevSlide() {
    selectSlide(view.index - 1);
  }

  function pointValue(slide: Slide, index: number, event: Event, commit: boolean) {
    const input = event.currentTarget;
    if (!(input instanceof HTMLInputElement)) return;
    const points = doc.at(slide).points;
    if (commit) points.set(index, input.value);
    else points.preview(index, input.value);
  }

  function handleKeyDown(event: KeyboardEvent) {
    const typing = event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement;
    if (isPresenting) {
      if (event.key === "ArrowRight" || event.key === " " || event.key === "PageDown") {
        event.preventDefault();
        nextSlide();
      } else if (event.key === "ArrowLeft" || event.key === "PageUp") {
        event.preventDefault();
        prevSlide();
      } else if (event.key === "Escape") {
        event.preventDefault();
        stopPresenting();
      }
    } else if (!typing && event.altKey && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      event.preventDefault();
      move(view.index, event.key === "ArrowUp" ? -1 : 1);
    } else if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault();
      startPresenting();
    }
  }

  const formattedTimer = $derived.by(() => {
    const mins = Math.floor(timerSeconds / 60);
    const secs = timerSeconds % 60;
    return `${String(mins).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
  });

  onDestroy(() => { if (timerInterval) clearInterval(timerInterval); });
</script>

{#snippet slideLayouts(slide: Slide, editing: boolean)}
  {#if slide.layout === "title"}
    <div class="layout-title-view">
      {#if editing}
        <input class="title-tag-input" aria-label="Category Tag" use:bindText={doc.at(slide).tag} placeholder="TOPIC / TAG" />
        <input class="title-hero-input" aria-label="Hero Title" use:bindText={doc.at(slide).title} placeholder="Presentation Title" />
        <input class="title-sub-input" aria-label="Subtitle" use:bindText={doc.at(slide).subtitle} placeholder="Supporting description or takeaway" />
      {:else}
        <span class="title-tag-input">{slide.tag}</span>
        <h1 class="title-hero-input">{slide.title}</h1>
        <p class="title-sub-input">{slide.subtitle}</p>
      {/if}
    </div>
  {:else if slide.layout === "split"}
    <div class="layout-split-view">
      {#if editing}
        <input class="slide-heading-input" aria-label="Slide Heading" use:bindText={doc.at(slide).title} placeholder="Slide Heading" />
      {:else}
        <h2 class="slide-heading-input">{slide.title}</h2>
      {/if}
      <div class="split-columns">
        <div class="bullets-list">
          {#each slide.points as point, index}
            {#if editing}
              <input class="bullet-item-input" aria-label="Bullet Point" value={point} placeholder="Bullet point..." oninput={(event) => pointValue(slide, index, event, false)} onchange={(event) => pointValue(slide, index, event, true)} />
            {:else}
              <div class="bullet-item-input">{point}</div>
            {/if}
          {/each}
        </div>
        <div class="highlight-card">
          {#if editing}
            <input class="highlight-tag" aria-label="Highlight Tag" use:bindText={doc.at(slide).highlightLabel} placeholder="TAG" />
            <input class="highlight-number" aria-label="Highlight Value" use:bindText={doc.at(slide).highlightValue} placeholder="Stat / Value" />
            <textarea class="highlight-text" aria-label="Highlight Description" use:bindText={doc.at(slide).highlightDesc} placeholder="Key impact note..." rows="2"></textarea>
          {:else}
            <span class="highlight-tag">{slide.highlightLabel}</span>
            <div class="highlight-number">{slide.highlightValue}</div>
            <div class="highlight-text">{slide.highlightDesc}</div>
          {/if}
        </div>
      </div>
    </div>
  {:else if slide.layout === "metric"}
    <div class="layout-metric-view">
      {#if editing}
        <input class="title-tag-input" aria-label="Category Tag" use:bindText={doc.at(slide).tag} placeholder="CATEGORY" />
        <input class="giant-metric-input" aria-label="Metric Stat" use:bindText={doc.at(slide).metricValue} placeholder="10x" />
        <input class="metric-label-input" aria-label="Metric Label" use:bindText={doc.at(slide).metricLabel} placeholder="KEY OUTCOME / METRIC" />
        <input class="title-sub-input" aria-label="Metric Description" use:bindText={doc.at(slide).subtitle} placeholder="Contextual description of this metric" style="text-align: center;" />
      {:else}
        <span class="title-tag-input">{slide.tag}</span>
        <div class="giant-metric-input">{slide.metricValue}</div>
        <div class="metric-label-input">{slide.metricLabel}</div>
        <p class="title-sub-input" style="text-align: center;">{slide.subtitle}</p>
      {/if}
    </div>
  {:else if slide.layout === "quote"}
    <div class="layout-quote-view">
      {#if editing}
        <textarea class="quote-textarea" aria-label="Quote Text" use:bindText={doc.at(slide).quoteText} placeholder="Enter memorable quote or principle..." rows="3"></textarea>
        <input class="quote-author-input" aria-label="Quote Author" use:bindText={doc.at(slide).author} placeholder="— Author, Source" />
      {:else}
        <blockquote class="quote-textarea">{slide.quoteText}</blockquote>
        <div class="quote-author-input">— {slide.author}</div>
      {/if}
    </div>
  {:else if slide.layout === "cards"}
    <div class="layout-cards-view">
      {#if editing}
        <input class="slide-heading-input" aria-label="Slide Heading" use:bindText={doc.at(slide).title} placeholder="Framework or Pillars" />
      {:else}
        <h2 class="slide-heading-input">{slide.title}</h2>
      {/if}
      <div class="cards-grid">
        {#each slide.cards as card (card.$id)}
          <div class="pillar-card">
            {#if editing}
              <input class="pillar-title-input" aria-label="Card Title" use:bindText={doc.at(card).title} placeholder="Pillar Title" />
              <textarea class="pillar-desc-textarea" aria-label="Card Description" use:bindText={doc.at(card).desc} placeholder="Description of this pillar..." rows="4"></textarea>
            {:else}
              <h3 class="pillar-title-input">{card.title}</h3>
              <p class="pillar-desc-textarea">{card.desc}</p>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}
{/snippet}

<svelte:window onkeydown={handleKeyDown} />


  <main class="deck-canvas" data-theme={doc.current.theme}>
    <article class="studio-shell">
      <header class="studio-header">
        <input class="deck-title-input" aria-label="Deck Title" use:bindText={doc.fields.deckTitle} />
        <div class="header-controls" data-slop-export="hide">
          <div class="layout-picker" aria-label="Slide Layout">
            {#each layouts as layout}
              <button type="button" class="layout-btn" class:active={currentSlide?.layout === layout} onclick={() => changeLayout(layout)}>
                {layout.toUpperCase()}
              </button>
            {/each}
          </div>
          <button type="button" class="theme-btn" title="Change Theme" onclick={cycleTheme}>{doc.current.theme.toUpperCase()}</button>
          <button type="button" class="present-btn" title="Start Presentation (Cmd+Enter)" onclick={startPresenting}>
            <Play size={13} fill="#ffffff" />
            <span>PRESENT</span>
          </button>
        </div>
      </header>

      <section class="studio-workspace">
        <aside class="filmstrip-sidebar" data-slop-export="hide" aria-label="Slide Filmstrip">
          <div class="filmstrip-header">
            <span>SLIDES ({doc.current.slides.length})</span>
            <button type="button" class="add-slide-btn" title="Add Slide" onclick={add}><Plus size={12} /></button>
          </div>
          <div class="slides-list">
            {#each doc.current.slides as slide, index (slide.$id)}
              <div
                class="filmstrip-thumb"
                class:active={index === view.index}
                onclick={() => selectSlide(index)}
                role="button"
                tabindex="0"
                onkeydown={(event) => event.key === "Enter" && selectSlide(index)}
              >
                <div class="thumb-meta">
                  <span>{String(index + 1).padStart(2, "0")} · {slide.layout.toUpperCase()}</span>
                  {#if doc.current.slides.length > 1}
                    <button type="button" class="thumb-delete-btn" title="Delete slide" onclick={(event) => { event.stopPropagation(); remove(index); }}>
                      <Trash2 size={11} />
                    </button>
                  {/if}
                </div>
                <span class="thumb-title">{slide.title || "Untitled"}</span>
              </div>
            {/each}
          </div>
        </aside>

        <section class="stage-viewport" aria-label="Active Slide Stage">
          <div class="slide-canvas-frame">
            {#if currentSlide}{@render slideLayouts(currentSlide, true)}{/if}
          </div>
        </section>
      </section>
    </article>

    <Dialog.Root bind:open={isPresenting}>
      <Dialog.Portal>
        <Dialog.Content class="presenter-overlay" data-theme={doc.current.theme} aria-label="Presenter Mode">
          <Dialog.Title class="sr-only">Presenter Mode</Dialog.Title>
          <div class="presenter-slide-scaler">
            <div class="slide-canvas-frame" style="max-width: 100%; height: 100%;">
              {#if currentSlide}{@render slideLayouts(currentSlide, false)}{/if}
            </div>
          </div>
          <nav class="presenter-hud" data-slop-export="hide" aria-label="Presenter Controls">
            <button type="button" class="hud-btn" onclick={prevSlide} disabled={view.index === 0}>
              <ChevronLeft size={16} />
            </button>
            <span>{view.index + 1} / {doc.current.slides.length}</span>
            <button type="button" class="hud-btn" onclick={nextSlide} disabled={view.index === view.last}>
              <ChevronRight size={16} />
            </button>
            <span style="opacity: 0.4;">|</span>
            <span>⏱ {formattedTimer}</span>
            <span style="opacity: 0.4;">|</span>
            <Dialog.Close class="hud-btn" title="Exit Presenter (Esc)" onclick={stopPresenting}>
              <X size={16} />
            </Dialog.Close>
          </nav>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  </main>
