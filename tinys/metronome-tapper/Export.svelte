<script lang="ts">
import "@hitslop/tinys/base.css";
import "./styles.css";
import type { CaptureProps } from "@hitslop/tinys/svelte";
import type schema from "./schema";
import { signatures } from "./schema";
import { metronomeView } from "./metronome";
let { snapshot }: CaptureProps<typeof schema> = $props();
const CAPTURE_ANGLE = -16;
const view = $derived(metronomeView(snapshot));
</script>

<main class="metronome-shell" aria-label="Exported metronome">
    <header class="chassis-head">
      <span class="screw" aria-hidden="true"></span>
      <p>PRECISION TEMPO</p>
      <span class="screw" aria-hidden="true"></span>
    </header>

    <section class="display-card">
      <div class="display-readout">
        <strong class="bpm-digits">{snapshot.bpm}</strong>
        <span class="bpm-unit">BPM</span>
      </div>
      <p class="tempo-descriptor">{view.tempoName}</p>
      <div class="beat-lights" aria-label={`${view.beatsPerBar} beats per bar`}>
        {#each Array(view.beatsPerBar) as _, index}
          <span class="beat-dot" class:accent={index === 0}></span>
        {/each}
      </div>
    </section>

    <section class="pendulum-chamber" aria-hidden="true">
      <div class="scale-grooves">
        <span>200</span>
        <span>160</span>
        <span>120</span>
        <span>90</span>
        <span>60</span>
      </div>
      <div class="pendulum-arm" style:transform="rotate({CAPTURE_ANGLE}deg)" style:--weight={view.weight}>
        <div class="brass-rod"></div>
        <div class="brass-weight"><i></i></div>
        <div class="pendulum-pivot"></div>
      </div>
    </section>

    <section class="controls-panel">
      <div class="settings-row">
        <div class="time-sig-selector" aria-label="Time signature">
          {#each signatures as value}
            <span class="sig-btn" data-state={snapshot.signature === value ? "checked" : undefined}>{value}</span>
          {/each}
        </div>
        <span class="mute-toggle" data-state={snapshot.muted ? "on" : undefined}>{snapshot.muted ? "MUTE" : "CLICK"}</span>
      </div>
    </section>
  </main>
