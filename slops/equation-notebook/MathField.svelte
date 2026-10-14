<script lang="ts" module>
  import { MathfieldElement } from "mathlive";
  // No CDN under slop://: the KaTeX fonts are declared in styles.css from bundled copies
  // (with --ML__static-fonts set, MathLive never fetches them) and the sounds are off.
  MathfieldElement.fontsDirectory = null;
  MathfieldElement.soundsDirectory = null;
  MathfieldElement.keypressSound = null;
  MathfieldElement.plonkSound = null;
</script>

<script lang="ts">
  import { onMount } from "svelte";

  let { latex, label, oninput, oncommit, onenter, onfocus }: {
    latex: string;
    label: string;
    /** Every keystroke, for a live preview. */
    oninput?: (latex: string) => void;
    /** Blur, Enter or Tab: the value worth saving. */
    oncommit: (latex: string) => void;
    onenter?: () => void;
    onfocus?: () => void;
  } = $props();

  let host: HTMLDivElement;
  let field = $state.raw<MathfieldElement>();
  let committed = "";

  function commit(value: string) {
    if (value === committed) return;
    committed = value;
    oncommit(value);
  }

  onMount(() => {
    committed = latex;
    const mf = new MathfieldElement();
    mf.setAttribute("aria-label", label);
    mf.value = latex;
    host.append(mf);
    // Options that need the mounted field.
    mf.mathVirtualKeyboardPolicy = "manual";
    mf.smartFence = true;
    mf.menuItems = [];
    const input = () => oninput?.(mf.value);
    const change = () => commit(mf.value);
    const keydown = (event: KeyboardEvent) => {
      if (event.key !== "Enter" || event.shiftKey || event.isComposing) return;
      event.preventDefault();
      event.stopPropagation();
      commit(mf.value);
      onenter?.();
    };
    mf.addEventListener("input", input);
    mf.addEventListener("change", change);
    mf.addEventListener("focus", () => onfocus?.());
    mf.addEventListener("keydown", keydown, { capture: true });
    field = mf;
    return () => {
      commit(mf.value);
      mf.removeEventListener("input", input);
      mf.removeEventListener("change", change);
      mf.removeEventListener("keydown", keydown, { capture: true });
      mf.remove();
      field = undefined;
    };
  });

  // The saved line can change underneath (undo, an agent's command): show it unless the
  // person is typing in this field.
  $effect(() => {
    const next = latex;
    if (!field || field.value === next) return;
    if (field.hasFocus()) return;
    committed = next;
    field.setValue(next, { silenceNotifications: true });
  });

  export function focus() { field?.focus(); }
</script>

<div class="math-host" bind:this={host}></div>
