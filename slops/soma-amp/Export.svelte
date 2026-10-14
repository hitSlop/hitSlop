<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { receiverView } from "./settings";

  let { snapshot }: CaptureProps<typeof schema> = $props();

  const view = $derived(receiverView(snapshot));
</script>

<article class="soma-export">
  <header><span>LISTENER-SUPPORTED RADIO / SOMAFM</span><strong>SOMA<span>AMP</span></strong></header>
  <div class="soma-export-display">
    <span>SELECTED STATION</span><h1>{view.station.title}</h1><p>{view.station.genre}</p>
    <div class="soma-spectrum" aria-hidden="true">{#each [22, 43, 66, 91, 58, 76, 100, 69, 40, 24] as height}<i style={`height: ${height}%`}></i>{/each}</div>
  </div>
  <dl><div><dt>Skin</dt><dd>{view.skinName}</dd></div><div><dt>Receiver</dt><dd>{view.volume}% · EQ {view.eqOn ? "on" : "off"}</dd></div></dl>
</article>
