<script lang="ts">
  import doc from "./schema";
  import { ui } from "./ui.svelte";
  import { dateLabel, dayKey, duration, elapsed, entryTitle, timeLabel } from "./journal";
  import { caregiverInitials, caregiverLabel } from "./shared";
  import { type Entry } from "./schema";
  import Mark from "./Mark.svelte";

  const title = $derived(doc.current.name.trim() ? `${doc.current.name.trim()}’s little days` : "Little Days");
  const initials = $derived(doc.current.name.trim().slice(0, 1).toLocaleUpperCase());
  const ordered = $derived([...doc.current.entries].sort((a, b) => b.timestamp - a.timestamp || a.$id.localeCompare(b.$id)));
  const dayEntries = $derived(ordered.filter(entry => dayKey(entry.timestamp) === ui.selectedDay));
  const feeds = $derived(dayEntries.filter(entry => entry.kind === "feeding").length);
  const diapers = $derived(dayEntries.filter(entry => entry.kind === "diaper").length);
  const sleepMinutes = $derived(dayEntries.reduce((total, entry) => total + (entry.kind === "sleep" && entry.endTimestamp !== undefined && entry.endTimestamp > entry.timestamp ? (entry.endTimestamp - entry.timestamp) / 60000 : 0), 0));
</script>

<article class="bj-export" data-color={doc.current.color}>
  <header class="bj-export-header">{@render portrait()}<div><p class="bj-eyebrow">Little Days · a baby journal</p><h1>{title}</h1><p>{dateLabel(ui.selectedDay)}</p></div><Mark kind="sun" size={66}/></header>
  <div class="bj-export-summary"><span>{feeds} {feeds === 1 ? "feed" : "feeds"}</span><span>{diapers} diaper {diapers === 1 ? "change" : "changes"}</span><span>{duration(sleepMinutes)} sleep</span></div>
  {#each dayEntries as entry (entry.$id)}{@render row(entry, false)}{:else}<p class="bj-export-empty">No moments recorded for this day.</p>{/each}
  <footer class="bj-footer"><span>Little moments. Lovingly kept.</span><Mark kind="heart" size={17}/></footer>
</article>

{#snippet portrait(large = false)}
  <div class="bj-portrait" class:bj-portrait-large={large}>
    {#if ui.photoUrl}<img src={ui.photoUrl} alt={doc.current.name.trim() ? `Photo of ${doc.current.name.trim()}` : "Baby photo"}/>
    {:else if initials}<span>{initials}</span>
    {:else}<Mark kind="sun" size={large ? 104 : 68}/>{/if}
  </div>
{/snippet}

{#snippet row(entry: Entry, editable: boolean)}
  <article class="bj-entry" data-kind={entry.kind}>
    <time class="bj-entry-time" datetime={Number.isFinite(new Date(entry.timestamp).getTime()) ? new Date(entry.timestamp).toISOString() : undefined}>{timeLabel(entry.timestamp)}</time>
    <span class="bj-entry-mark"><Mark kind={entry.kind} size={34}/></span>
    <div class="bj-entry-copy"><h3>{entryTitle(entry)}</h3>
      {#if entry.kind === "sleep" && entry.endTimestamp !== undefined}<p class="bj-sleep-end">Until {timeLabel(entry.endTimestamp)}{dayKey(entry.endTimestamp) !== dayKey(entry.timestamp) ? ` · ${dateLabel(dayKey(entry.endTimestamp))}` : ""}</p>{/if}
      {#if entry.notes}<p class="bj-entry-note">{entry.notes}</p>{/if}
    </div>
    <div class="bj-by"><span class="bj-person-dot">{caregiverInitials(entry.caregiverId)}</span><span>{caregiverLabel(entry.caregiverId)}</span></div>

  </article>
{/snippet}
