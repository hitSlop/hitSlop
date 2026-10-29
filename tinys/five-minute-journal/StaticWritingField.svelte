<script lang="ts">let { value, prefix = "" }: { value: string; prefix?: string; label?: string; placeholder?: string } = $props();</script>
<div class="fmj-line">{#if prefix}<span class="fmj-linePrefix" aria-hidden="true">{prefix}</span>{/if}<p class="fmj-lineText">{value}</p></div>
