<script lang="ts">
import { useDocument } from "@hitslop/document/svelte";
import schema from "./schema";
import { checked, sets } from "./workout";
const doc = useDocument(schema);
const total = $derived(doc.current.exercises.reduce((sum, ex) => sum + sets(ex), 0));
const done = $derived(doc.current.exercises.reduce((sum, ex) => sum + checked(ex).length, 0));
const marks = $derived(total ? Math.round((done / total) * 4) : 0);
</script>

<svg class="icon" viewBox="0 0 512 512" fill="none" aria-hidden="true">
  <rect x="24" y="30" width="464" height="458" rx="64" fill="#050607" />
  <rect x="24" y="22" width="464" height="458" rx="64" fill="#191c1f" stroke="#343b3e" stroke-width="3" />
  <path d="M85 75h110" stroke="#c3f653" stroke-width="9" stroke-linecap="round" />
  <circle cx="256" cy="242" r="139" fill="#080a0b" stroke="#343b3e" stroke-width="8" />
  <circle cx="256" cy="236" r="117" fill="#282d30" stroke="#464e50" stroke-width="3" />
  <circle cx="256" cy="236" r="81" fill="#141719" stroke="#080a0b" stroke-width="12" />
  <circle cx="256" cy="236" r="32" fill="#080a0b" stroke="#66705c" stroke-width="7" />
  {#each [0, 120, 240] as angle}
    <rect x="230" y="134" width="52" height="27" rx="13" transform={`rotate(${angle} 256 236)`} fill="#080a0b" stroke="#59644f" stroke-width="3" />
  {/each}
  {#each [0, 1, 2, 3] as index}
    <rect x={100 + index * 82} y="402" width="66" height="28" rx="7" fill={index < marks ? "#c3f653" : "#303739"} stroke={index < marks ? "#c3f653" : "#505a53"} stroke-width="2" />
  {/each}
</svg>
