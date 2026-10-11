<script lang="ts">
  import doc from "./schema";
  import Chamber from "./Chamber.svelte";
  import { intakeView } from "./intake";

  const view = $derived(intakeView(doc.current));
  const percent = $derived(view.percent);
</script>

<article class="exportFlask" aria-label="Exported water tracker">
  <header class="header">
    <span class="brand">Hydration 01</span>
    <span class="target">Target {doc.current.target} {doc.current.unit}</span>
  </header>
  <Chamber level={view.fill} {percent} interactive={false} />
  <footer class="foot"><span>{doc.current.logs.length ? `${doc.current.logs.length} drinks logged` : "No drinks logged."}</span></footer>
</article>
