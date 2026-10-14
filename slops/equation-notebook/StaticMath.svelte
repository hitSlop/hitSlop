<script lang="ts">
  import { readableMath } from "./readable-math";
  let { latex, class: className = "" }: { latex: string; class?: string } = $props();
</script>

<span class={["static-math", className]}>{readableMath(latex)}</span>
