<script lang="ts">
import { useDocument } from "@hitslop/document/svelte";
import schema, { type Slide } from "./schema";
const doc = useDocument(schema);
const currentSlide = $derived(doc.current.slides[doc.current.activeSlideIndex] ?? doc.current.slides[0]);
</script>
{#snippet slideLayouts(slide: Slide)}
  {#if slide.layout === "title"}
    <div class="layout-title-view">
      <span class="title-tag-input">{slide.tag}</span>
      <h1 class="title-hero-input">{slide.title}</h1>
      <p class="title-sub-input">{slide.subtitle}</p>
    </div>
  {:else if slide.layout === "split"}
    <div class="layout-split-view">
      <h2 class="slide-heading-input">{slide.title}</h2>
      <div class="split-columns">
        <div class="bullets-list">
          {#each slide.points as point}
            <div class="bullet-item-input">{point}</div>
          {/each}
        </div>
        <div class="highlight-card">
          <span class="highlight-tag">{slide.highlightLabel}</span>
          <div class="highlight-number">{slide.highlightValue}</div>
          <div class="highlight-text">{slide.highlightDesc}</div>
        </div>
      </div>
    </div>
  {:else if slide.layout === "metric"}
    <div class="layout-metric-view">
      <span class="title-tag-input">{slide.tag}</span>
      <div class="giant-metric-input">{slide.metricValue}</div>
      <div class="metric-label-input">{slide.metricLabel}</div>
      <p class="title-sub-input" style="text-align: center;">{slide.subtitle}</p>
    </div>
  {:else if slide.layout === "quote"}
    <div class="layout-quote-view">
      <blockquote class="quote-textarea">{slide.quoteText}</blockquote>
      <div class="quote-author-input">— {slide.author}</div>
    </div>
  {:else if slide.layout === "cards"}
    <div class="layout-cards-view">
      <h2 class="slide-heading-input">{slide.title}</h2>
      <div class="cards-grid">
        {#each slide.cards as card (card.$id)}
          <div class="pillar-card">
            <h3 class="pillar-title-input">{card.title}</h3>
            <p class="pillar-desc-textarea">{card.desc}</p>
          </div>
        {/each}
      </div>
    </div>
  {/if}
{/snippet}
<article class="slide-canvas-frame" data-theme={doc.current.theme}>
  {#if currentSlide}{@render slideLayouts(currentSlide)}{/if}
</article>
