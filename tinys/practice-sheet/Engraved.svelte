<script lang="ts">
  // The engraved score. abcjs draws into this element from the stored ABC text; the
  // Score it returns (timings, SVG positions) lives only as long as this view.
  import { untrack } from "svelte";
  import { engrave, type Score } from "./score";

  let { abc, width = 680, delay = 0, onscore, onpick }: {
    abc: string;
    width?: number;
    /** Wait this long after the text settles before re-engraving (while typing). */
    delay?: number;
    onscore?: (score: Score | null, element: HTMLElement) => void;
    onpick?: (measure: number, extend: boolean) => void;
  } = $props();

  let element: HTMLDivElement;
  let engraved = false;

  $effect(() => {
    const text = abc, staff = width;
    const draw = () => untrack(() => {
      let score: Score | null = null;
      try { score = engrave(element, text, { width: staff, onPick: onpick }); }
      catch { element.replaceChildren(); score = null; }
      engraved = true;
      onscore?.(score, element);
    });
    if (!engraved || !untrack(() => delay)) { draw(); return; }
    const timer = setTimeout(draw, untrack(() => delay));
    return () => clearTimeout(timer);
  });
  $effect(() => () => untrack(() => { element?.replaceChildren(); onscore?.(null, element); }));
</script>

<div class="ps-engraved" bind:this={element}></div>
