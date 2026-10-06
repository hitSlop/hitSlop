<script lang="ts">
  import doc from "./schema";
  import { CHANNEL_IDS } from "./schema";
  import { CHANNELS } from "./shared";

  const data = $derived(doc.current);
  const solo = $derived(CHANNEL_IDS.some((id) => data.soloed[id]));
  const live = $derived(CHANNEL_IDS.some((id) => {
  if (data.muted[id]) return false;
  const audible = solo ? data.soloed[id] : true;
  return audible && data.channels[id] > 0;
}));
  const statusLabel = $derived(!data.playing ? "Standby" : !live ? "Silent" : solo ? "Solo" : "Live");
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
        <span class="power" data-on={data.playing} aria-hidden="true"></span>
      </div>
    </header>
    <section class="deck" aria-label="Channel mix">
      {#each CHANNELS as ch}
        {@const IconComp = ch.icon}
        {@const level = Math.round(data.channels[ch.id])}
        {@const audible = data.muted[ch.id] ? false : solo ? data.soloed[ch.id] : true}
        <div class="strip" data-audible={audible} data-soloed={data.soloed[ch.id]}>
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
    <footer class="footer"><span class="status">{statusLabel}</span></footer>
  </div>
</article>
