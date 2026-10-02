<script lang="ts">
import { ui, fallbackChannels } from "./ui.svelte";
import doc from "./schema";
const chromeUrl = "/assets/alien-radio-chrome.png";
const selected = $derived(ui.channels.find(channel => channel.id === doc.current.selectedChannelId) ?? ui.channels[0] ?? fallbackChannels[0]!);
</script>

<article class="radio" aria-label="Exported Alien Radio">
      <img class="chrome" src={chromeUrl} alt="" />
      <header class="identity"><span class="lamp"></span><strong>ALIEN RADIO</strong><small>INTERSTELLAR RECEIVER / AR-01</small></header>
      <section class="display">
        <div class="topline"><span>{ui.status}</span><span>LOCAL INDEX</span><span>—</span></div>
        <div class="station"><span>broadcast</span><strong>{selected.title}</strong></div>
        <p class="track">{selected.lastPlaying || selected.description}</p>
      </section>
    </article>
