<script lang="ts">
import type { CaptureProps } from "hitslop/svelte";
import type schema from "./schema";
import Vessel from "./Vessel.svelte";
import { read } from "./model";
let { snapshot }: CaptureProps<typeof schema> = $props();
const reading = $derived(read(snapshot, Date.now()));
</script>

<div class="hourglass-icon" aria-hidden="true"><Vessel remaining={reading.remaining} solid /></div>
