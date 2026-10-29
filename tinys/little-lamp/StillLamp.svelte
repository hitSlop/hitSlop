<script lang="ts">let { on }: { on: boolean } = $props();</script>
<svg width="320" height="320" viewBox="0 0 320 320" aria-label={on ? "Little Lamp, awake" : "Little Lamp, asleep"}>
  <defs>
    <linearGradient id="lamp-shade" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#ffe093"/><stop offset="0.6" stop-color="var(--tiny-accent)"/><stop offset="1" stop-color="#bb7748"/></linearGradient>
    <linearGradient id="lamp-arm"><stop stop-color="#b27c48"/><stop offset="0.45" stop-color="#ffe7a8"/><stop offset="1" stop-color="#c5915c"/></linearGradient>
    <linearGradient id="lamp-base" x1="0" y1="0" x2="0.4" y2="1"><stop stop-color="#dda96a"/><stop offset="1" stop-color="#95653c"/></linearGradient>
  </defs>
  {#if on}<ellipse cx="160" cy="274" rx="142" ry="30" fill="var(--tiny-accent)" fill-opacity="0.16"/>{/if}
  <path d="M198 272 Q286 300 291 262" fill="none" stroke="#807760" stroke-width="3" stroke-linecap="round"/>
  <ellipse cx="154" cy="277" rx="65" ry="15" fill="url(#lamp-base)"/>
  <ellipse cx="148" cy="268" rx="38" ry="5" fill="#f8d68e"/>
  <path d={on ? "M154 264L95 201L140 145" : "M154 264L102 207L155 160"} fill="none" stroke="url(#lamp-arm)" stroke-width="13" stroke-linecap="round" stroke-linejoin="round"/>
  <circle cx={on ? 95 : 102} cy={on ? 201 : 207} r="10" fill="#b8844d"/>
  <g transform={on ? "rotate(-8 160 150)" : "rotate(7 160 150)"}>
    <circle cx="163" cy="170" r="15" fill={on ? "#fff5bf" : "#fff0bd"}/>
    <path d="M94 150 Q124 76 153 69 Q184 60 222 150 Q226 158 216 160 L100 160 Q88 158 94 150Z" fill="url(#lamp-shade)"/>
    <ellipse cx="156" cy="158" rx="66" ry="7" fill="#865331"/>
    <ellipse cx="124" cy="103" rx="5" ry="11" transform="rotate(-24 124 103)" fill="#ffeaba" fill-opacity="0.7"/>
    {#if on}<ellipse cx="140" cy="132" rx="3.2" ry="5" fill="#604225"/><ellipse cx="179" cy="132" rx="3.2" ry="5" fill="#604225"/>{:else}<path d="M137 134h6 M176 134h6" stroke="#604225" stroke-width="2" stroke-linecap="round"/>{/if}
    <ellipse cx="128" cy="144" rx="4" ry="2" fill="#db9f61"/><ellipse cx="191" cy="144" rx="4" ry="2" fill="#db9f61"/>
  </g>
</svg>
