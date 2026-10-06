<script lang="ts">
  import doc from "./schema";
  import { countdownState } from "./countdown";

  const view = $derived(countdownState(doc.current.targetDate, doc.current.targetTime, Date.now()));
  const value = $derived(view.kind === "today" ? "Today" : view.value.length > 4 ? "999+" : view.value);
</script>

<svg class="icon" viewBox="0 0 512 512" fill="none" aria-hidden="true">
  <rect x="22" y="24" width="468" height="466" rx="70" fill="#b9dcff" />
  <rect x="63" y="102" width="386" height="338" rx="34" fill="#25212c25" />
  <rect x="63" y="88" width="386" height="338" rx="34" fill="#d7eaff" />
  <rect x="63" y="76" width="386" height="338" rx="34" fill="#edf6ff" stroke="#25212c30" stroke-width="2" />
  {#each [151, 337] as x (x)}
    <rect {x} y="46" width="24" height="63" rx="12" fill="#c84c41" />
    <rect {x} y="42" width="24" height="63" rx="12" fill="#f45d4f" />
  {/each}
  <text x="256" y="264" text-anchor="middle" fill="#25212c" font-family="Countdown Fredoka, sans-serif" font-weight="550" font-size={value.length > 3 ? 94 : 132} letter-spacing="-2">{value}</text>
  <rect x="129" y="296" width="254" height="47" rx="8" fill="#f2d94e" />
  <text x="256" y="327" text-anchor="middle" fill="#25212c" font-family="Countdown Fredoka, sans-serif" font-size="25">{view.kind === "today" ? "It’s today!" : view.unit}</text>
</svg>
