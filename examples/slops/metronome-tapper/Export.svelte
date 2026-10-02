<script lang="ts">
import doc, { signatures } from "./schema";
const CAPTURE_ANGLE = -16;
const beatsPerBar = $derived(Number.parseInt(doc.current.signature.split("/")[0] ?? "4", 10));
const tempoName = $derived.by(() => {
  const bpm = doc.current.bpm;
  if (bpm < 60) return "Largo";
  if (bpm < 76) return "Adagio";
  if (bpm < 108) return "Andante";
  if (bpm < 120) return "Moderato";
  if (bpm < 156) return "Allegro";
  if (bpm < 200) return "Vivace";
  return "Presto";
});
const weight = $derived(`${((doc.current.bpm - 40) / 200) * 58 + 18}%`);
</script>

<main class="metronome-shell" aria-label="Exported metronome">
    <header class="chassis-head">
      <span class="screw" aria-hidden="true"></span>
      <p>PRECISION TEMPO</p>
      <span class="screw" aria-hidden="true"></span>
    </header>

    <section class="display-card">
      <div class="display-readout">
        <strong class="bpm-digits">{doc.current.bpm}</strong>
        <span class="bpm-unit">BPM</span>
      </div>
      <p class="tempo-descriptor">{tempoName}</p>
      <div class="beat-lights" aria-label={`${beatsPerBar} beats per bar`}>
        {#each Array(beatsPerBar) as _, index}
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
      <div class="pendulum-arm" style:transform="rotate({CAPTURE_ANGLE}deg)" style:--weight={weight}>
        <div class="brass-rod"></div>
        <div class="brass-weight"><i></i></div>
        <div class="pendulum-pivot"></div>
      </div>
    </section>

    <section class="controls-panel">
      <div class="settings-row">
        <div class="time-sig-selector" aria-label="Time signature">
          {#each signatures as value}
            <span class="sig-btn" data-state={doc.current.signature === value ? "checked" : undefined}>{value}</span>
          {/each}
        </div>
        <span class="mute-toggle" data-state={doc.current.muted ? "on" : undefined}>{doc.current.muted ? "MUTE" : "CLICK"}</span>
      </div>
    </section>
  </main>
