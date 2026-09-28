<script lang="ts">
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Play from "@lucide/svelte/icons/play";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import { Dialog } from "bits-ui";
  import { onDestroy } from "svelte";
  import schema, { layouts, themes, type Slide, type SlideLayout } from "./schema";

  const doc = useDocument(schema);
  let isPresenting = $state(false);
  let timerSeconds = $state(0);
  let timerInterval: ReturnType<typeof setInterval> | undefined;

  const currentSlide = $derived(doc.current.slides[doc.current.activeSlideIndex] ?? doc.current.slides[0]);

  function selectSlide(index: number) {
    const last = Math.max(0, doc.current.slides.length - 1);
    doc.fields.activeSlideIndex.set(Math.max(0, Math.min(last, index)));
  }

  function addSlide() {
    doc.change((tx) => {
      tx.fields.slides.insert({
        layout: "split",
        title: "New Slide Title",
        points: ["First key insight", "Second supporting point", "Third impactful takeaway"],
        highlightLabel: "METRIC",
        highlightValue: "100%",
        highlightDesc: "Description of the achievement or focus.",
        cards: [],
        notes: "Speaker talking points go here.",
      });
      tx.fields.activeSlideIndex.set(doc.current.slides.length);
    });
  }

  function deleteSlide(index: number) {
    const slides = doc.current.slides;
    const slide = slides[index];
    if (!slide || slides.length <= 1) return;
    const next = Math.min(doc.current.activeSlideIndex, slides.length - 2);
    doc.change((tx) => {
      tx.fields.slides.remove(slide.$id);
      tx.fields.activeSlideIndex.set(next);
    });
  }

  function moveSlide(index: number, direction: -1 | 1) {
    const slides = doc.current.slides;
    const slide = slides[index];
    const neighbor = slides[index + direction];
    if (!slide || !neighbor) return;
    const active = doc.current.activeSlideIndex;
    doc.change((tx) => {
      tx.fields.slides.move(slide.$id, direction < 0 ? { before: neighbor.$id } : { after: neighbor.$id });
      if (active === index) tx.fields.activeSlideIndex.set(index + direction);
      else if (active === index + direction) tx.fields.activeSlideIndex.set(index);
    });
  }

  function setLayout(layout: SlideLayout) {
    const slide = currentSlide;
    if (!slide || slide.layout === layout) return;
    doc.change((tx) => {
      const item = tx.at(slide);
      item.layout.set(layout);
      const text = (current: string | undefined, handle: { set(value: string): void }, value: string) => {
        if (current === undefined) handle.set(value);
      };
      if (layout === "title") {
        text(slide.tag, item.tag, "");
        text(slide.title, item.title, "");
        text(slide.subtitle, item.subtitle, "");
      } else if (layout === "split") {
        text(slide.title, item.title, "");
        if (!slide.points.length) {
          item.points.insert("Point 1");
          item.points.insert("Point 2");
          item.points.insert("Point 3");
        }
        text(slide.highlightLabel, item.highlightLabel, "HIGHLIGHT");
        text(slide.highlightValue, item.highlightValue, "Key Metric");
        text(slide.highlightDesc, item.highlightDesc, "");
      } else if (layout === "metric") {
        text(slide.tag, item.tag, "");
        text(slide.metricValue, item.metricValue, "99.9%");
        text(slide.metricLabel, item.metricLabel, "RELIABILITY");
        text(slide.subtitle, item.subtitle, "");
      } else if (layout === "quote") {
        text(slide.quoteText, item.quoteText, "Inspiration leads to innovation.");
        text(slide.author, item.author, "Author Name");
      } else {
        text(slide.title, item.title, "");
        if (!slide.cards.length) {
          item.cards.insert({ title: "Pillar 1", desc: "First pillar description." });
          item.cards.insert({ title: "Pillar 2", desc: "Second pillar description." });
          item.cards.insert({ title: "Pillar 3", desc: "Third pillar description." });
        }
      }
    });
  }

  function cycleTheme() {
    const next = themes[(themes.indexOf(doc.current.theme) + 1) % themes.length] ?? "swiss";
    doc.fields.theme.set(next);
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
    if (doc.current.activeSlideIndex < doc.current.slides.length - 1) doc.fields.activeSlideIndex.set(doc.current.activeSlideIndex + 1);
  }

  function prevSlide() {
    if (doc.current.activeSlideIndex > 0) doc.fields.activeSlideIndex.set(doc.current.activeSlideIndex - 1);
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
      moveSlide(doc.current.activeSlideIndex, event.key === "ArrowUp" ? -1 : 1);
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

<Slop>
  <main class="deck-canvas" data-theme={doc.current.theme}>
    <article class="studio-shell">
      <header class="studio-header">
        <input class="deck-title-input" aria-label="Deck Title" use:bindText={doc.fields.deckTitle} />
        <div class="header-controls" data-slop-export="hide">
          <div class="layout-picker" aria-label="Slide Layout">
            {#each layouts as layout}
              <button type="button" class="layout-btn" class:active={currentSlide?.layout === layout} onclick={() => setLayout(layout)}>
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
            <button type="button" class="add-slide-btn" title="Add Slide" onclick={addSlide}><Plus size={12} /></button>
          </div>
          <div class="slides-list">
            {#each doc.current.slides as slide, index (slide.$id)}
              <div
                class="filmstrip-thumb"
                class:active={index === doc.current.activeSlideIndex}
                onclick={() => selectSlide(index)}
                role="button"
                tabindex="0"
                onkeydown={(event) => event.key === "Enter" && selectSlide(index)}
              >
                <div class="thumb-meta">
                  <span>{String(index + 1).padStart(2, "0")} · {slide.layout.toUpperCase()}</span>
                  {#if doc.current.slides.length > 1}
                    <button type="button" class="thumb-delete-btn" title="Delete slide" onclick={(event) => { event.stopPropagation(); deleteSlide(index); }}>
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
            <button type="button" class="hud-btn" onclick={prevSlide} disabled={doc.current.activeSlideIndex === 0}>
              <ChevronLeft size={16} />
            </button>
            <span>{doc.current.activeSlideIndex + 1} / {doc.current.slides.length}</span>
            <button type="button" class="hud-btn" onclick={nextSlide} disabled={doc.current.activeSlideIndex === doc.current.slides.length - 1}>
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

  {#snippet exportView()}
    <article class="slide-canvas-frame" data-theme={doc.current.theme}>
      {#if currentSlide}{@render slideLayouts(currentSlide, false)}{/if}
    </article>
  {/snippet}
  {#snippet icon()}
    <section class="deck-icon" aria-hidden="true">
      <div class="icon-projector-shell">
        <div class="icon-screen-bezel">
          <div class="icon-slide-card">
            <div class="icon-slide-header">
              <span class="icon-tag-pill">PITCH DECK</span>
              <span class="icon-slide-num">01 / 05</span>
            </div>
            <div class="icon-slide-content">
              <div class="icon-giant-title">
                <span class="icon-title-bar primary"></span>
                <span class="icon-title-bar secondary"></span>
              </div>
              <div class="icon-content-split">
                <div class="icon-bullets">
                  <span class="icon-bullet-row"></span>
                  <span class="icon-bullet-row"></span>
                  <span class="icon-bullet-row short"></span>
                </div>
                <div class="icon-stat-card">
                  <span class="icon-stat-number">+12x</span>
                  <span class="icon-stat-label">VELOCITY</span>
                </div>
              </div>
            </div>
          </div>
        </div>
        <div class="icon-footer-controls">
          <div class="icon-filmstrip-dots">
            <span class="dot active"></span>
            <span class="dot"></span>
            <span class="dot"></span>
            <span class="dot"></span>
            <span class="dot"></span>
          </div>
          <div class="icon-present-badge">
            <span class="icon-play-triangle">▶</span>
            <span>PRESENT</span>
          </div>
        </div>
      </div>
    </section>
  {/snippet}
</Slop>
