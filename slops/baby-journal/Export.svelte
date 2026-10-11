<script lang="ts">
  import { onMount, tick } from "svelte";
  import { attachments, capture } from "hitslop/svelte";
  import doc from "./schema";
  import { dateLabel, dayKey, duration, entryTitle, journalDays, timeLabel, type Entry } from "./journal";
  import { caregiverInitials, caregiverLabel } from "./shared";
  import Mark from "./Mark.svelte";

  // The export is a fresh page: it cannot see the editor's day, so it shows every day.
  const today = dayKey(Date.now());
  const title = $derived(doc.current.name.trim() ? `${doc.current.name.trim()}’s little days` : "Little Days");
  const initials = $derived(doc.current.name.trim().slice(0, 1).toLocaleUpperCase());
  const people = $derived(doc.current.caregivers);
  const days = $derived(journalDays(doc.current));
  const photo = $derived(doc.current.photo);
  const photoUrl = $derived(photo ? attachments.url(photo.id) : "");

  onMount(() => capture.onPrepare(async () => {
    await tick();
    await Promise.all(Array.from(document.images, (image) => image.decode().catch(() => undefined)));
  }));
</script>

<article class="bj-export" data-color={doc.current.color}>
  <header class="bj-export-header">{@render portrait()}<div><p class="bj-eyebrow">Little Days · a baby journal</p><h1>{title}</h1><p>{dateLabel(today)}</p></div><Mark kind="sun" size={66}/></header>
  {#each days as group (group.day)}
    <section class="bj-export-day" aria-label={dateLabel(group.day)}>
      <h3>{dateLabel(group.day)}</h3>
      <div class="bj-export-summary"><span>{group.summary.feeds} {group.summary.feeds === 1 ? "feed" : "feeds"}</span><span>{group.summary.diapers} diaper {group.summary.diapers === 1 ? "change" : "changes"}</span><span>{duration(group.summary.sleepMinutes)} sleep</span></div>
      {#each group.entries as entry (entry.$id)}{@render row(entry)}{/each}
    </section>
  {:else}<p class="bj-export-empty">No moments recorded yet.</p>{/each}
  <footer class="bj-footer"><span>Little moments. Lovingly kept.</span><Mark kind="heart" size={17}/></footer>
</article>

{#snippet portrait(large = false)}
  <div class="bj-portrait" class:bj-portrait-large={large}>
    {#if photoUrl}<img src={photoUrl} alt={doc.current.name.trim() ? `Photo of ${doc.current.name.trim()}` : "Baby photo"}/>
    {:else if initials}<span>{initials}</span>
    {:else}<Mark kind="sun" size={large ? 104 : 68}/>{/if}
  </div>
{/snippet}

{#snippet row(entry: Entry)}
  <article class="bj-entry" data-kind={entry.kind}>
    <time class="bj-entry-time" datetime={Number.isFinite(new Date(entry.timestamp).getTime()) ? new Date(entry.timestamp).toISOString() : undefined}>{timeLabel(entry.timestamp)}</time>
    <span class="bj-entry-mark"><Mark kind={entry.kind} size={34}/></span>
    <div class="bj-entry-copy"><h3>{entryTitle(entry)}</h3>
      {#if entry.kind === "sleep" && entry.endTimestamp !== undefined}<p class="bj-sleep-end">Until {timeLabel(entry.endTimestamp)}{dayKey(entry.endTimestamp) !== dayKey(entry.timestamp) ? ` · ${dateLabel(dayKey(entry.endTimestamp))}` : ""}</p>{/if}
      {#if entry.notes}<p class="bj-entry-note">{entry.notes}</p>{/if}
    </div>
    <div class="bj-by"><span class="bj-person-dot">{caregiverInitials(people, entry.caregiverId)}</span><span>{caregiverLabel(people, entry.caregiverId)}</span></div>
  </article>
{/snippet}
