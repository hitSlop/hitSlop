<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { Button, Tabs } from "bits-ui";
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import { Slop, useDocument } from "@hitslop/document/svelte";
  import { capture } from "@hitslop/document/capture";
  import schema from "./schema";
  import type { Session } from "./src/countdown.svelte";
  import { createCountdown } from "./src/countdown.svelte";
  import Dial from "./Dial.svelte";

  const doc = useDocument(schema);
  let pendingSession = $state<Session | null>(null);
  let saveFailed = $state(false);
  const clock = createCountdown(
    () => ({ focusMinutes: doc.current.focusMinutes, restMinutes: doc.current.restMinutes }),
    saveSession,
  );
  const clockState = clock.state;
  const running = $derived(clockState.status === "running");
  const label = $derived(`${String(Math.floor(clockState.remaining / 60)).padStart(2, "0")}:${String(clockState.remaining % 60).padStart(2, "0")}`);
  const remainingRatio = $derived(Math.max(0, Math.min(1, clockState.remaining / clockState.duration)));
  const arc = new Tween(untrack(() => remainingRatio), { duration: 350, easing: cubicOut });
  let initialized = false;
  const completed = $derived(doc.current.history.filter((session) => session.kind === "focus").length);
  const action = $derived(running ? "Pause" : clockState.status === "paused" ? "Resume" : "Start");
  const status = $derived(clockState.status === "paused" ? "Paused · take your time"
    : running ? clockState.kind === "focus" ? "Time to focus" : "Take a little break"
      : clockState.status === "complete" ? clockState.completedKind === "focus" ? "Focus done. Break ready." : "Break done. Focus ready."
        : "Ready when you are");

  function saveSession(session: Session) {
    try {
      const history = doc.current.history;
      const expired = history.slice(11).map((entry) => entry.$id);
      doc.change((tx) => {
        tx.fields.history.insert(
          session,
          history.length ? { before: history[0].$id } : undefined,
        );
        for (const id of expired) tx.fields.history.remove(id);
      }, { message: "Complete timer session" });
      pendingSession = null;
      saveFailed = false;
    } catch {
      pendingSession = session;
      saveFailed = true;
    }
  }

  function retrySession() {
    if (pendingSession) saveSession(pendingSession);
  }

  $effect(() => {
    if (!running) return;
    const interval = setInterval(clock.advance, 250);
    return () => clearInterval(interval);
  });
  $effect(() => {
    const instant = !initialized || prefersReducedMotion.current;
    void arc.set(remainingRatio, { duration: instant ? 0 : 350, delay: 0 });
    initialized = true;
  });
  onMount(() => capture.onPrepare(async () => {
    await arc.set(remainingRatio, { duration: 0, delay: 0 });
    await tick();
  }));
  onDestroy(() => { void arc.set(arc.target, { duration: 0, delay: 0 }); });
</script>

{#snippet leaf()}
  <div class="pom-leaf" aria-hidden="true"><i class="pom-leaf-blade"></i><i class="pom-leaf-blade"></i><i class="pom-leaf-blade"></i><b class="pom-leaf-stem"></b></div>
{/snippet}

<Slop>
  <main class="pom-shell" data-kind={clockState.kind} data-status={clockState.status} aria-label="Pomodoro timer">
    {@render leaf()}
    <section class="pom-stage" aria-label="Timer">
      <Dial label={label} ratio={arc.current} kind={clockState.kind}>
        {#snippet modes()}
          <Tabs.Root value={clockState.kind} onValueChange={clock.select}>
            <Tabs.List class="pom-mode-switch" aria-label="Timer mode" data-slop-export="hide">
              <Tabs.Trigger class="pom-mode-trigger" value="focus" disabled={running}>Focus <span class="pom-mode-duration">{doc.current.focusMinutes}m</span></Tabs.Trigger>
              <Tabs.Trigger class="pom-mode-trigger" value="rest" disabled={running}>Break <span class="pom-mode-duration">{doc.current.restMinutes}m</span></Tabs.Trigger>
            </Tabs.List>
          </Tabs.Root>
        {/snippet}
        {#snippet detail()}
          <p class="pom-status" role="status">{status}</p>
        {/snippet}
      </Dial>
      <div class="pom-controls" data-slop-export="hide">
        <Button.Root class="pom-start-button" onclick={() => running ? clock.pause() : clock.start()} aria-label={`${action} timer`}>
          {#if running}<Pause class="pom-start-icon" fill="currentColor" aria-hidden="true" />{:else}<Play class="pom-start-icon" fill="currentColor" aria-hidden="true" />{/if}
          <span>{action}</span>
        </Button.Root>
        <Button.Root class="pom-reset-button" onclick={clock.reset} aria-label="Reset timer" title="Reset timer"><RotateCcw class="pom-reset-icon" aria-hidden="true" /></Button.Root>
      </div>
    </section>
    {#if saveFailed}
      <p class="pom-footer pom-error" role="alert">Session not saved. <button class="pom-retry" onclick={retrySession}>Retry</button></p>
    {:else}
      <p class="pom-footer" aria-label={`${completed} focus sessions in the last ${doc.current.history.length} saved sessions`}><span class="pom-count-dot" aria-hidden="true"></span>{completed} recent focus {completed === 1 ? "session" : "sessions"}</p>
    {/if}
  </main>

  {#snippet exportView()}
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
  {/snippet}

  {#snippet icon()}
    <div class="pom-icon" aria-hidden="true">
      <article class="pom-shell">
        {@render leaf()}
        <div class="pom-icon-face"><span class="pom-icon-digits">{doc.current.focusMinutes}</span><i class="pom-icon-tick"></i></div>
        <div class="pom-icon-button"></div>
      </article>
    </div>
  {/snippet}

</Slop>
