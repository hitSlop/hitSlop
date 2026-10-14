<script lang="ts">
  import { Canvas } from "@threlte/core";
  import * as THREE from "three";
  import { onMount } from "svelte";
  import DieGlyph from "./DieGlyph.svelte";
  import TrayScene from "./TrayScene.svelte";
  import { DiceKit, numeralFont, readPalette, type Palette, type ShownRoll } from "./dice3d";

  let { roll, reduced, label, onRolling, onSettled }: {
    roll: ShownRoll | undefined;
    reduced: boolean;
    label: string;
    onRolling?: (id: string) => void;
    onSettled?: (id: string) => void;
  } = $props();

  let host: HTMLDivElement;
  let kit = $state.raw<DiceKit>();
  let palette = $state.raw<Palette>();

  function hasWebGL(): boolean {
    try {
      const probe = document.createElement("canvas");
      const gl = probe.getContext("webgl2") ?? probe.getContext("webgl");
      gl?.getExtension("WEBGL_lose_context")?.loseContext();
      return !!gl;
    } catch {
      return false;
    }
  }
  const webgl = hasWebGL();

  onMount(() => {
    let alive = true;
    let made: DiceKit | undefined;
    if (!webgl && roll) onSettled?.(roll.id);
    const theme = new MutationObserver(() => {
      if (!made) return;
      palette = readPalette(host);
      made.setPalette(palette);
    });
    // The numerals are painted onto the dice, so wait for the bundled face first.
    void document.fonts.load(`700 64px ${numeralFont}`).catch(() => []).then(() => {
      if (!alive || !webgl) return;
      palette = readPalette(host);
      made = new DiceKit(palette);
      kit = made;
      theme.observe(document.documentElement, { attributes: true, attributeFilter: ["style", "class", "data-theme"] });
    });
    return () => {
      alive = false;
      theme.disconnect();
      made?.dispose();
    };
  });
</script>

<div bind:this={host} class="tray-stage" role="img" aria-label={label}>
  {#if webgl}
    {#if kit && palette}
      <Canvas
        renderMode="on-demand"
        shadows={THREE.PCFShadowMap}
        toneMapping={THREE.NoToneMapping}
        dpr={Math.min(window.devicePixelRatio || 1, 2)}
        createRenderer={(canvas) => new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, powerPreference: "low-power" })}
      >
        <TrayScene {roll} {kit} {palette} {reduced} {onRolling} {onSettled} />
      </Canvas>
    {/if}
  {:else if roll}
    <div class="tray-flat">
      {#each roll.dice as die, index (index)}<DieGlyph kind={die.kind} value={die.value} size={58} />{/each}
    </div>
  {/if}
</div>
