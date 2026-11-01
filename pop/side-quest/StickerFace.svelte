<script lang="ts">
  import { emoji, type Sticker } from "./quest";

  let { kind }: { kind: Sticker } = $props();
</script>

<span class="sq-sticker-face">{emoji[kind]}</span>
