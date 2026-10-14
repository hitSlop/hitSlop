<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { CHANNELS } from "./shared";
  import { mixerView } from "./mixer";
  let { snapshot }: CaptureProps<typeof schema> = $props();

  const data = $derived(snapshot);
  // Playback is not saved, so the export shows the mix as it would sound when switched on.
  const view = $derived(mixerView(data));
</script>

<article class="exportCanvas" aria-label="Exported ambient mixer">
  <div class="chassis">
    <header class="header">
      <div class="brand">
        <strong>Atmos 01</strong>
        <span>{data.preset}</span>
      </div>
      <div class="headerRight">
        <div class="masterDial"><span>Master {Math.round(data.master)}</span></div>
      </div>
    </header>
    <section class="deck" aria-label="Channel mix">
      {#each CHANNELS as ch (ch.id)}
        {@const IconComp = ch.icon}
        {@const level = Math.round(data.channels[ch.id])}
        <div class="strip" data-audible={view.audible[ch.id]} data-soloed={data.soloed[ch.id]}>
          <div class="channelInfo">
            <IconComp size={15} />
            <span class="channelName">{ch.label}</span>
          </div>
          <div class="faderWell">
            <span class="faderGroove"><span class="faderRange" style:height="{level}%"></span></span>
            <span class="staticCap" style:bottom="{Math.max(6, Math.min(90, level))}%"></span>
          </div>
          <div class="channelBottom">
            <span class="channelLevel">{level}</span>
            <div class="padRow">
              <span class="pad" data-kind="mute" data-state={data.muted[ch.id] ? "on" : "off"}>M</span>
              <span class="pad" data-kind="solo" data-state={data.soloed[ch.id] ? "on" : "off"}>S</span>
            </div>
          </div>
        </div>
      {/each}
    </section>
    <footer class="footer"><span class="status">{view.status}</span></footer>
  </div>
</article>
