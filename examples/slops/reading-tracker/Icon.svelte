<script lang="ts">
import doc from "./schema";
const totalBooks = $derived(doc.current.books.length);
const readBooks = $derived(doc.current.books.filter((book) => book.status === "Read").length);
</script>

<div class="iconSurface" aria-hidden="true">
      <svg class="iconGraphic" viewBox="0 0 512 512" fill="none">
        <rect x="28" y="28" width="456" height="456" rx="104" fill="var(--slop-stamp)"/>
        <rect x="102" y="111" width="76" height="294" rx="12" fill="#d8c9ed"/>
        <path d="M120 148H160M120 367H160" stroke="var(--slop-stamp)" stroke-width="8" stroke-linecap="round"/>
        <rect x="194" y="87" width="100" height="318" rx="12" fill="var(--slop-paper)"/>
        <path d="M225 87H263V222L244 207L225 222Z" fill="#d994b5"/>
        <rect x="308" y="144" width="72" height="261" rx="12" transform="rotate(-9 308 144)" fill="#ead7e3"/>
        <path d="M91 416H429" stroke="#d994b5" stroke-width="12" stroke-linecap="round"/>
        {#if totalBooks > 0 && readBooks === totalBooks}<path d="M219 322L242 345L274 300" stroke="var(--slop-read)" stroke-width="13" stroke-linecap="round" stroke-linejoin="round"/>{/if}
      </svg>
    </div>
