<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import chromeUrl from "./assets/alien-radio-chrome.png";
  import { radioView } from "./radio";

  let { snapshot }: CaptureProps<typeof schema> = $props();

  // Exports never reach the network: the station resolves against the bundled index.
  const view = $derived(radioView(snapshot));
</script>

<article class="radio" aria-label="Exported Alien Radio">
  <img class="chrome" src={chromeUrl} alt="" />
  <header class="identity"><span class="lamp"></span><strong>ALIEN RADIO</strong><small>INTERSTELLAR RECEIVER / AR-01</small></header>
  <section class="display">
    <div class="topline"><span>STANDBY</span><span>LOCAL INDEX</span><span>—</span></div>
    <div class="station"><span>{view.selected.genre.split("|")[0] || "broadcast"}</span><strong>{view.selected.title}</strong></div>
    <p class="track">{view.selected.lastPlaying || view.selected.description}</p>
  </section>
</article>
