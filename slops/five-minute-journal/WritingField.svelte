<script lang="ts">
  import type { Handle, Text } from "hitslop";
  import { bindText } from "hitslop/svelte";
  import { tick } from "svelte";

  let { value, label, placeholder = "", prefix = "", text }: {
    value: string;
    label: string;
    placeholder?: string;
    prefix?: string;
    text: Handle<Text>;
  } = $props();

  function grow(node: HTMLTextAreaElement, _value: string) {
    let alive = true;
    const resize = () => {
      if (!alive) return;
      node.style.height = "auto";
      node.style.height = `${node.scrollHeight}px`;
    };
    let width = 0;
    const observer = new ResizeObserver((entries) => {
      const nextWidth = entries[0]?.contentRect.width ?? 0;
      if (nextWidth !== width) {
        width = nextWidth;
        resize();
      }
    });
    observer.observe(node);
    void document.fonts.ready.then(resize);
    resize();
    return {
      update() { void tick().then(resize); },
      destroy() {
        alive = false;
        observer.disconnect();
      },
    };
  }
</script>

<div class={"fmj-line"}>
  {#if prefix}<span class={"fmj-linePrefix"} aria-hidden="true">{prefix}</span>{/if}
  <textarea class={"fmj-writingField"} aria-label={label} {placeholder} rows={1} use:bindText={text} use:grow={value}></textarea>
</div>
