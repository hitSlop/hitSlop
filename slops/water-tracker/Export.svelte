<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import Chamber from "./Chamber.svelte";
  import { intakeView } from "./intake";

  let { snapshot }: CaptureProps<typeof schema> = $props();
  const view = $derived(intakeView(snapshot));
  const percent = $derived(view.percent);
</script>

<article class="exportFlask" aria-label="Exported water tracker">
  <header class="header">
    <span class="brand">Hydration 01</span>
    <span class="target">Target {snapshot.target} {snapshot.unit}</span>
  </header>
  <Chamber level={view.fill} {percent} total={view.total} unit={snapshot.unit} interactive={false} />
  <footer class="foot"><span>{snapshot.logs.length ? `${snapshot.logs.length} drinks logged` : "No drinks logged."}</span></footer>
</article>
