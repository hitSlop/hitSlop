<script lang="ts">
import type { Snippet } from "svelte";
let { seconds=1500, label="A little time for you.", children }: { seconds?: number; label?: string; children?: Snippet }=$props();
</script>
<div class="tomato"><svg class="leaf" viewBox="0 0 100 50" aria-hidden="true"><path d="M50 42C20 45 7 21 7 21L42 30 37 2 55 25 83 7 70 31 97 30Q76 49 50 42" fill="#346b43"/><path d="M52 33Q46 13 59 3" fill="none" stroke="#285437" stroke-width="7" stroke-linecap="round"/></svg><div class="face"><span class="eyebrow">FOCUS TIME</span><strong>{Math.floor(seconds/60).toString().padStart(2,"0")}:{Math.floor(seconds%60).toString().padStart(2,"0")}</strong><span class="label">{label}</span>{#if children}{@render children()}{/if}</div></div>
