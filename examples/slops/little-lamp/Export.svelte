<script lang="ts">
  import doc from "./schema";
  import Lamp from "./Lamp.svelte";
</script>

<div class="lamp-export" role="img" aria-label={doc.current.on ? "Little Lamp, awake" : "Little Lamp, asleep"}>
  <Lamp on={doc.current.on} still />
</div>
