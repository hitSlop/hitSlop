<script lang="ts">
  import doc from "./schema";
  import { worldView } from "./world";
  const view = $derived(worldView(doc.current));
  const land = $derived(doc.current.season === "winter" ? "var(--slop-snow)" : doc.current.season === "autumn" ? "var(--slop-autumn)" : "var(--slop-grass)");
  const position = (lat: number, lon: number) => ({ x: 240 + Math.sin(lon) * Math.cos(lat) * 146, y: 248 + Math.sin(lat) * 135 });
</script>
<figure class="planet-export">
  <svg width="480" height="480" viewBox="0 0 480 480" role="img" aria-label={`${view.title}, a world in ${view.season}`}>
    <rect width="480" height="480" fill="var(--slop-space)"/>
    {#each Array.from({length: 32}, (_, i) => i) as i}<circle cx={(i * 137 + doc.current.seed) % 480} cy={(i * 83 + doc.current.seed * 7) % 480} r={i % 3 === 0 ? 1.5 : 0.8} fill="var(--slop-snow)" fill-opacity="0.7"/>{/each}
    <circle cx="240" cy="248" r="160" fill="var(--slop-ocean)"/>
    <path d="M113 151L175 105L216 118L231 169L204 201L215 252L166 276L146 247L99 226L88 190Z M250 306L284 239L338 221L391 250L377 320L328 377L286 389L263 351Z M262 110L310 113L344 145L314 179L271 171Z" fill={land}/>
    <path d="M88 190L160 207L146 247 M175 105L181 163L231 169 M286 389L310 306L377 320 M284 239L310 306L250 306" fill="none" stroke="var(--slop-forest)" stroke-opacity="0.2" stroke-width="2"/>
    {#each [...doc.current.props].sort((a, b) => a.lat - b.lat) as prop}
      {@const at = position(prop.lat, prop.lon)}
      <g transform={`translate(${at.x} ${at.y})`}>
        {#if prop.kind === "tree"}<rect x="-3" y="-16" width="6" height="22" fill="var(--slop-trunk)"/><path d="M0 -47L-17 -14H17Z M0 -35L-21 -3H21Z" fill={doc.current.season === "spring" ? "var(--slop-blossom)" : doc.current.season === "winter" ? "var(--slop-snow)" : "var(--slop-forest)"}/>
        {:else if prop.kind === "house"}<rect x="-15" y="-25" width="30" height="27" fill="var(--slop-wall)"/><path d="M-20 -24L0 -43L20 -24Z" fill="var(--slop-roof)"/><rect x="-4" y="-13" width="8" height="15" fill="var(--slop-trunk)"/>
        {:else if prop.kind === "crystal"}<path d="M0 -31L10 -15L7 3L-8 3L-12 -16Z" fill="var(--slop-crystal)"/><path d="M0 -31V3L10 -15Z" fill="var(--slop-snow)" fill-opacity="0.4"/>
        {:else}<path d="M-14 2L-16 -9L-5 -19L10 -15L17 2Z" fill="var(--slop-rock)"/>{/if}
      </g>
    {/each}
  </svg>
  <figcaption><strong>{view.title}</strong><span>{view.things} · {view.season}</span></figcaption>
</figure>
