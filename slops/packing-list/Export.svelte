<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { packingView } from "./items";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  // The export is a fresh page with no filter: it always lists every category and item.
  const view = $derived(packingView(snapshot));
</script>

<article class="export-tag" aria-label="Exported packing list">
  <div class="airmail" aria-hidden="true"></div>
  <header class="header">
    <div class="eyelet-row">
      <span class="flag-btn" aria-hidden="true"><span class="flag-emoji">{snapshot.flag || "🇯🇵"}</span></span>
      <span class="airline">HS-BAG // TRAVEL MANIFEST</span>
    </div>
    <div class="meta">
      <div class="field">
        <span class="meta-label">TRIP / EXPEDITION</span>
        <strong class="meta-input title-input">{snapshot.tripTitle || "Untitled trip"}</strong>
      </div>
      <div class="submeta">
        <div class="field">
          <span class="meta-label">DESTINATION</span>
          <strong class="meta-input dest-input">{snapshot.destination}</strong>
        </div>
        <div class="field">
          <span class="meta-label">PASSENGER</span>
          <strong class="meta-input passenger-input">{snapshot.traveler}</strong>
        </div>
        <div class="field">
          <span class="meta-label">DEPARTURE</span>
          <strong class="meta-input date-input">{snapshot.departureDate}</strong>
        </div>
      </div>
    </div>
  </header>
  <section class="readiness">
    <div class="readiness-row">
      <div class="readiness-status">
        <span>BAGGAGE STATUS:</span>
        <span class="ready-badge" data-complete={view.complete}>{view.packed} / {view.total} PACKED ({view.percent}%)</span>
      </div>
    </div>
    <div class="track"><div class="fill" data-complete={view.complete} style:width="{view.percent}%"></div></div>
  </section>
  <div class="export-list">
    {#each (preview ? (view.sections).slice(0, 3) : view.sections) as section (section.category.$id)}
      {#if section.items.length > 0}
        <section class="block" aria-label={section.category.name}>
          <div class="block-head">
            <span class="stamp" data-stamp={section.category.color}>{section.category.tagCode} // {section.category.name}</span>
            <span class="block-count">{section.packed} of {section.total} packed</span>
          </div>
          <ul class="list">
            {#each (preview ? (section.items).slice(0, 4) : section.items) as item (item.$id)}
              <li class="row" data-packed={item.packed}>
                <span data-checkbox-root data-state={item.packed ? "checked" : "unchecked"}>{#if item.packed}<span class="check-icon">✓</span>{/if}</span>
                <span class="qty">×{item.quantity}</span>
                <span class="item-text">{item.text || "Untitled item"}</span>
                <span class="star" data-active={item.essential}>{item.essential ? "★" : ""}</span>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    {:else}
      <div class="empty"><h2>Nothing to pack yet.</h2></div>
    {/each}
  </div>
  <footer class="footer">
    <div class="barcode-block">
      <div class="barcode" aria-hidden="true"></div>
      <span class="barcode-code">{snapshot.bagTag} // CHECKED AIRLINE TAG</span>
    </div>
    <div class="footer-stamp" data-complete={view.complete}>{view.complete ? "100% READY FOR FLIGHT" : "PACK IN PROGRESS"}</div>
  </footer>
</article>
