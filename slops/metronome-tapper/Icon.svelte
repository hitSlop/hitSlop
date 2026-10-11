<script lang="ts">
import doc from "./schema";
import { metronomeView } from "./metronome";
const view = $derived(metronomeView(doc.current));
</script>

<div style="width:512px;height:512px;display:grid;place-items:center" aria-hidden="true">
    <article class="metronome-icon">
      <div class="icon-top-plate">
        <span class="icon-brand-dot"></span>
        <span class="icon-brand-text">TEMPO INSTRUMENT</span>
      </div>
      <div class="icon-face">
        <div class="icon-bpm-card">
          <span class="icon-bpm-num">{doc.current.bpm}</span>
          <span class="icon-bpm-lbl">{view.tempoName}</span>
        </div>
        <div class="icon-pendulum-chamber">
          <div class="icon-scale-lines">
            <i></i><i></i><i></i><i></i><i></i><i></i><i></i>
          </div>
          <div class="icon-rod">
            <div class="icon-weight"></div>
          </div>
        </div>
      </div>
      <div class="icon-controls">
        <span class="icon-tap-btn">TAP</span>
        <span class="icon-play-btn">▶</span>
      </div>
    </article>
  </div>
