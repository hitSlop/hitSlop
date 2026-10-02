<script lang="ts">
  import doc from "./schema";
  import Dial from "./Dial.svelte";
  import { useClock } from "./timer.svelte";

  const clockState = useClock().state;
  const label = $derived(`${String(Math.floor(clockState.remaining / 60)).padStart(2, "0")}:${String(clockState.remaining % 60).padStart(2, "0")}`);
  const remainingRatio = $derived(Math.max(0, Math.min(1, clockState.remaining / clockState.duration)));
  const completed = $derived(doc.current.history.filter((session) => session.kind === "focus").length);
</script>

<article class="pom-shell pom-export-shell" data-kind={clockState.kind} aria-label="Pomodoro snapshot">
  {@render leaf()}
  <section class="pom-stage">
    <Dial label={label} ratio={remainingRatio} kind={clockState.kind}>
      {#snippet detail()}<p class="pom-status">Time remaining</p>{/snippet}
    </Dial>
  </section>
  <p class="pom-export-name">Pomodoro</p>
  <p class="pom-footer"><span class="pom-count-dot" aria-hidden="true"></span>{completed} recent focus {completed === 1 ? "session" : "sessions"}</p>
</article>

{#snippet leaf()}
  <div class="pom-leaf" aria-hidden="true"><i class="pom-leaf-blade"></i><i class="pom-leaf-blade"></i><i class="pom-leaf-blade"></i><b class="pom-leaf-stem"></b></div>
{/snippet}
