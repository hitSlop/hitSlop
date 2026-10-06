<script lang="ts">
  import { ui } from "./ui.svelte";
  import doc from "./schema";
  import { type PackingCategory, type PackingItem } from "./schema";

  const categories = $derived(exportCategories());
  const packed = $derived(doc.current.items.filter((item) => item.packed).length);
  const total = $derived(doc.current.items.length);
  const percent = $derived(total > 0 ? Math.round((packed / total) * 100) : 0);
  const complete = $derived(total > 0 && packed === total);

  function countsFor(key: string): { packed: number; total: number } {
    const items = doc.current.items.filter((item) => item.key === key);
    return { total: items.length, packed: items.filter((item) => item.packed).length };
  }

  function itemsFor(key: string): PackingItem[] {
    return doc.current.items.filter((item) => {
      if (item.key !== key) return false;
      if (ui.filter === "remaining") return !item.packed;
      return true;
    });
  }

  function exportCategories(): readonly PackingCategory[] {
    return ui.filter === "all" || ui.filter === "remaining"
      ? doc.current.categories
      : doc.current.categories.filter((category) => category.key === ui.filter);
  }
</script>

<article class="export-tag" aria-label="Exported packing list">
  <div class="airmail" aria-hidden="true"></div>
  <header class="header">
    <div class="eyelet-row">
      <span class="flag-btn" aria-hidden="true"><span class="flag-emoji">{doc.current.flag || "🇯🇵"}</span></span>
      <span class="airline">HS-BAG // TRAVEL MANIFEST</span>
    </div>
    <div class="meta">
      <div class="field">
        <span class="meta-label">TRIP / EXPEDITION</span>
        <strong class="meta-input title-input">{doc.current.tripTitle || "Untitled trip"}</strong>
      </div>
      <div class="submeta">
        <div class="field">
          <span class="meta-label">DESTINATION</span>
          <strong class="meta-input dest-input">{doc.current.destination}</strong>
        </div>
        <div class="field">
          <span class="meta-label">PASSENGER</span>
          <strong class="meta-input passenger-input">{doc.current.traveler}</strong>
        </div>
        <div class="field">
          <span class="meta-label">DEPARTURE</span>
          <strong class="meta-input date-input">{doc.current.departureDate}</strong>
        </div>
      </div>
    </div>
  </header>
  <section class="readiness">
    <div class="readiness-row">
      <div class="readiness-status">
        <span>BAGGAGE STATUS:</span>
        <span class="ready-badge" data-complete={complete}>{packed} / {total} PACKED ({percent}%)</span>
      </div>
    </div>
    <div class="track"><div class="fill" data-complete={complete} style:width="{percent}%"></div></div>
  </section>
  <div class="export-list">
    {#each categories as category (category.$id)}
      {@const items = itemsFor(category.key)}
      {@const counts = countsFor(category.key)}
      {#if items.length > 0}
        <section class="block" aria-label={category.name}>
          <div class="block-head">
            <span class="stamp" data-stamp={category.color}>{category.tagCode} // {category.name}</span>
            <span class="block-count">{counts.packed} of {counts.total} packed</span>
          </div>
          <ul class="list">
            {#each items as item (item.$id)}
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
      <span class="barcode-code">{doc.current.bagTag} // CHECKED AIRLINE TAG</span>
    </div>
    <div class="footer-stamp" data-complete={complete}>{complete ? "100% READY FOR FLIGHT" : "PACK IN PROGRESS"}</div>
  </footer>
</article>
