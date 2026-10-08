<script lang="ts">
  import doc from "./schema";
  import Lamp from "./Lamp.svelte";
</script>

<div class="lamp-icon"><Lamp on={doc.current.on} still /></div>
