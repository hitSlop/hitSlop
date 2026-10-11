<script lang="ts">
  // Typeset LaTeX without an editor, for the export and the icon. MathLive's markup uses
  // the bundled KaTeX fonts and mathlive/static.css (both imported once by App.svelte).
  import { convertLatexToMarkup } from "mathlive";
  let { latex, class: className = "" }: { latex: string; class?: string } = $props();
  const markup = $derived.by(() => {
    try { return convertLatexToMarkup(latex); } catch { return ""; }
  });
</script>

<span class={["static-math", className]}>{#if markup}{@html markup}{:else}<code>{latex}</code>{/if}</span>
