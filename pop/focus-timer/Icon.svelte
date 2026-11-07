<script lang="ts">
import type { CaptureProps } from "@hitslop/pop/svelte";
import type schema from "./schema";
import Dial from "./Dial.svelte";
import "./styles.css";
let {snapshot}: CaptureProps<typeof schema>=$props();
</script>
<div style="display:flex;width:320px;height:320px;align-items:center;justify-content:center"><Dial seconds={snapshot.minutes*60}/></div>
