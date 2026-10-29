<script lang="ts">
  import { ZONE_LABEL, clock, dayLabel, type Session, type Zone } from "./meter";
  // The last saved session, shared by the editor and the export.
  let { session, zone }: { session: Session | undefined; zone: Zone | undefined } = $props();
</script>

{#if session}
  <dl class="qm-summary">
    <div><dt>Day</dt><dd>{dayLabel(session.date)}</dd></div>
    <div><dt>Peak</dt><dd data-zone={zone}>{session.peak} {zone ? ZONE_LABEL[zone] : ""}</dd></div>
    <div><dt>Listened</dt><dd>{clock(session.seconds)}</dd></div>
    <div><dt>Too loud</dt><dd>{clock(session.loudSeconds)}</dd></div>
    <div><dt>Episodes</dt><dd>{session.episodes}</dd></div>
  </dl>
{:else}
  <p class="qm-none">No session saved yet. Start listening, then stop to save a summary.</p>
{/if}
