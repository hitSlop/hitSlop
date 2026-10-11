<script lang="ts">
  import "./styles.css";
  // The export shows the last saved snapshot as it was kept, or the resting mirror when
  // none is saved. It never asks for the camera or microphone.
  import { attachments } from "hitslop/svelte";
  import doc from "./schema";
  import { mirrorView } from "./mirror";
  import Glass from "./Glass.svelte";

  const view = $derived(mirrorView(doc.current));
  const ratio = $derived(view.snapshot ? `${view.snapshot.width} / ${view.snapshot.height}` : "4 / 3");
</script>

<article class="fg-export">
  <div class="fg-export-glass" style:aspect-ratio={ratio}>
    <Glass variant="export" image={view.snapshot ? attachments.url(view.snapshot.id) : null} alt="The fogged mirror, as saved" />
  </div>
  <p class="fg-export-caption"><span class="fg-export-name">Fog</span>{view.snapshot ? view.caption : "breathe on the mirror, then press S to keep a frame"}</p>
</article>
