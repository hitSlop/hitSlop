<script lang="ts">
import { onMount } from "svelte";
import { getDocument } from "@hitslop/pop/svelte";
import schema from "./schema";
import Dial from "./Dial.svelte";
import "./styles.css";
const doc = getDocument(schema);
let remaining = $state(1500), deadline = $state(0), running = $state(false);
onMount(() => {
 remaining = doc.current.minutes * 60;
 const update = () => { if (running) { remaining = Math.max(0, Math.ceil((deadline-Date.now())/1000)); if (!remaining) running=false; } };
 let timer: ReturnType<typeof setInterval> | undefined;
 const visibility = () => { clearInterval(timer); update(); if (!document.hidden) timer = setInterval(update,250); };
 visibility();
 document.addEventListener("visibilitychange",visibility);
 return () => { clearInterval(timer); document.removeEventListener("visibilitychange",visibility); };
});
function toggle() {
 if (running) remaining=Math.max(0,Math.ceil((deadline-Date.now())/1000));
 else { if (!remaining) remaining=doc.current.minutes*60; deadline=Date.now()+remaining*1000; }
 running=!running;
}
</script>
<main><Dial seconds={remaining} label={remaining===0 ? "Nicely done." : "A little time for you."}><div class="controls"><button onclick={toggle}>{running ? "Ⅱ Pause" : "▶ Start"}</button><button class="reset" aria-label="Reset timer" onclick={() => { running=false; remaining=doc.current.minutes*60; }}>↺</button></div></Dial></main>
