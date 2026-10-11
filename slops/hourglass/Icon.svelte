<script lang="ts">
import doc from "./schema";
import Vessel from "./Vessel.svelte";
import { read } from "./model";
const reading = $derived(read(doc.current, Date.now()));
</script>

<div class="hourglass-icon" aria-hidden="true"><Vessel remaining={reading.remaining} solid /></div>
