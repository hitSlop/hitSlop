<script lang="ts">
  import doc from "./schema";
  import { stations } from "./stations";

  const selected = $derived(stations.find(s => s.id === doc.current.selectedStationId) ?? stations[0]!);

  const skinName = $derived(doc.current.skin.name ?? "Classic base skin");
</script>

<article class="soma-export">
  <header><span>LISTENER-SUPPORTED RADIO / SOMAFM</span><strong>SOMA<span>AMP</span></strong></header>
  <div class="soma-export-display">
    <span>SELECTED STATION</span><h1>{selected.title}</h1><p>{selected.genre}</p>
    <div class="soma-spectrum" aria-hidden="true">{#each [22, 43, 66, 91, 58, 76, 100, 69, 40, 24] as height}<i style={`height: ${height}%`}></i>{/each}</div>
  </div>
  <dl><div><dt>Skin</dt><dd>{skinName}</dd></div><div><dt>Receiver</dt><dd>{Math.round(doc.current.volume)}% · EQ {doc.current.equalizer.on ? "on" : "off"}</dd></div></dl>
</article>
