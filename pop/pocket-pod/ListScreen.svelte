<script lang="ts" module>
export type Row = { label: string; value?: string; chevron?: boolean };
</script>

<script lang="ts">
import type { Snippet } from "svelte";
import TitleBar from "./TitleBar.svelte";

let { title, rows, cursor, aside, footer, empty, playing, paused, held, touch = false, onrow }: {
  title: string;
  rows: readonly Row[];
  cursor: number;
  aside?: Snippet;
  footer?: string;
  empty?: string;
  playing?: boolean;
  paused?: boolean;
  held?: boolean;
  touch?: boolean;
  onrow: (index: number) => void;
} = $props();

const visible = 7;
const offset = $derived(touch ? 0 : Math.max(0, Math.min(cursor - 3, rows.length - visible)));
const shown = $derived(touch ? rows : rows.slice(offset, offset + visible));
function reveal(node: HTMLLIElement, selected: boolean) {
  const update = (active: boolean) => { if (active) node.scrollIntoView({ block: "nearest" }); };
  update(selected);
  return { update };
}
</script>

<section class="pod-screen pod-list" aria-label={title}>
  <TitleBar {title} {playing} {paused} {held} />
  <div class="pod-list-body" data-aside={aside ? "yes" : "no"}>
    <ul class="pod-rows" role="listbox" aria-label={title}>
      {#each shown as row, index (offset + index)}
        {@const at = offset + index}
        <li role="option" aria-selected={at === cursor} data-selected={at === cursor} use:reveal={at === cursor}>
          <button type="button" onclick={() => onrow(at)}>
            <span class="pod-row-label">{row.label}</span>
            {#if row.value}<span class="pod-row-value">{row.value}</span>{/if}
            {#if row.chevron}<span class="pod-row-chevron" aria-hidden="true">›</span>{/if}
          </button>
        </li>
      {:else}
        <li class="pod-rows-empty">{empty ?? "Nothing here yet"}</li>
      {/each}
    </ul>
    {#if aside}<aside class="pod-aside">{@render aside()}</aside>{/if}
  </div>
  {#if footer}<footer class="pod-footer">{footer}</footer>{/if}
</section>
