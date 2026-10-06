<script lang="ts">
  import { emoji } from "./quest";
  import type { Sticker } from "./schema";

  let { kind }: { kind: Sticker } = $props();
</script>

<span class="sq-sticker-face">{emoji[kind]}</span>
