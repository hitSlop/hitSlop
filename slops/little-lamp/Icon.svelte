<script lang="ts">
  import doc from "./schema";
  import Lamp from "./StillLamp.svelte";
</script>

<div class="lamp-icon"><Lamp on={doc.current.on} /></div>
