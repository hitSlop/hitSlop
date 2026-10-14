<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { CHANNELS } from "./shared";
  import { getDocument } from "hitslop/svelte";
  import { onMount, untrack } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { Select, Toggle, Button } from "bits-ui";
  import Power from "@lucide/svelte/icons/power";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Check from "@lucide/svelte/icons/check";
  import schema, { CHANNEL_IDS } from "./schema";
  import { AmbientEngine } from "./engine";
  import { applyPreset, mixerView, PRESET_NAMES, setLevel, setMuted, setSoloed, type ChannelID } from "./mixer";

  const doc = getDocument(schema);
  const PRESET_ITEMS = PRESET_NAMES.map((name) => ({ value: name, label: name }));
  const travel = {
    rain: new Tween(untrack(() => doc.current.channels.rain), { duration: 0, easing: cubicOut }),
    thunder: new Tween(untrack(() => doc.current.channels.thunder), { duration: 0, easing: cubicOut }),
    wind: new Tween(untrack(() => doc.current.channels.wind), { duration: 0, easing: cubicOut }),
    birds: new Tween(untrack(() => doc.current.channels.birds), { duration: 0, easing: cubicOut }),
    night: new Tween(untrack(() => doc.current.channels.night), { duration: 0, easing: cubicOut }),
  } satisfies Record<ChannelID, Tween<number>>;

  // Playback is view state: the audio graph lives only while this window is open, and a
  // reopened mix waits for the power button (WebAudio needs a user gesture).
  const engine = new AmbientEngine();
  let playing = $state(false);
  let glide = false;
  let vuLevel = $state(0);
  let vuTimer: ReturnType<typeof setInterval> | null = null;

  const view = $derived(mixerView(doc.current, playing));
  const presetItems = $derived(doc.current.preset === "Custom" ? [{ value: "Custom", label: "Custom" }, ...PRESET_ITEMS] : PRESET_ITEMS);

  function shown(id: ChannelID): number {
    return Math.round(travel[id].current);
  }

  function choosePreset(name: string): void {
    glide = true;
    doc.change((tx) => applyPreset(tx, name)).catch((error) => {
      glide = false;
      throw error;
    });
  }

  function dragLevel(id: ChannelID, input: HTMLInputElement): void {
    doc.fields.channels[id].preview(Number(input.value));
  }

  function commitLevel(id: ChannelID, input: HTMLInputElement): void {
    void doc.change((tx) => setLevel(tx, doc.current, id, Number(input.value)));
  }

  async function togglePower(): Promise<void> {
    if (!(await engine.ensure())) return;
    playing = !playing;
  }

  $effect(() => {
    const duration = glide && !prefersReducedMotion.current ? 320 : 0;
    glide = false;
    for (const id of CHANNEL_IDS) void travel[id].set(doc.current.channels[id], { duration, delay: 0 });
  });

  $effect(() => {
    const { levels } = view;
    const master = doc.current.master;
    if (engine.isOpen) engine.setMix(playing, master, levels);
  });

  function paintVu(): void {
    const energy = mixerView(doc.current, playing).energy;
    if (!playing || prefersReducedMotion.current) {
      vuLevel = Math.round(energy * 10);
      return;
    }
    vuLevel = Math.max(0, Math.min(10, Math.round(energy * 10 + (Math.random() * 2 - 1))));
  }

  onMount(() => {
    vuTimer = setInterval(paintVu, 120);
    return () => {
      if (vuTimer) clearInterval(vuTimer);
      for (const id of CHANNEL_IDS) void travel[id].set(travel[id].target, { duration: 0, delay: 0 });
      engine.close();
    };
  });
</script>

<main class="canvas" aria-label="Ambient sound mixer">
  <article class="chassis">
    <header class="header">
      <div class="brand"><strong>Atmos 01</strong></div>
      <Select.Root type="single" value={doc.current.preset} items={presetItems} onValueChange={(value) => { if (value && value !== "Custom") choosePreset(value); }}>
        <Select.Trigger class="presetTrigger" aria-label="Mixer preset" data-slop-export="hide">
          <Select.Value placeholder="Preset" />
          <ChevronDown size={13} />
        </Select.Trigger>
        <Select.Portal>
          <Select.Content class="presetContent" sideOffset={6}>
            <Select.Viewport>
              {#each presetItems as item (item.value)}
                <Select.Item value={item.value} label={item.label}>
                  {#snippet children({ selected })}
                    <span>{item.label}</span>
                    {#if selected}<Check size={13} />{/if}
                  {/snippet}
                </Select.Item>
              {/each}
            </Select.Viewport>
          </Select.Content>
        </Select.Portal>
      </Select.Root>

      <div class="headerRight">
        <div class="masterDial">
          <span>Master</span>
          <div class="masterSlider">
            <span class="masterTrack"><span class="masterRange" style:width="{doc.current.master}%"></span></span>
            <span class="masterThumb" style:left="{doc.current.master}%"></span>
            <input class="sliderInput" type="range" min="0" max="100" step="1" aria-label="Master volume" bind:value={doc.fields.master.value} />
          </div>
        </div>
        <Button.Root type="button" class="power" data-on={playing} onclick={() => void togglePower()} aria-pressed={playing} aria-label={playing ? "Stop mix" : "Start mix"}>
          <Power size={16} />
        </Button.Root>
      </div>
    </header>

    <section class="deck" aria-label="Audio channel faders">
      {#each CHANNELS as ch (ch.id)}
        {@const IconComp = ch.icon}
        {@const level = shown(ch.id)}
        {@const muted = doc.current.muted[ch.id]}
        {@const soloed = doc.current.soloed[ch.id]}
        <div class="strip" data-audible={view.audible[ch.id]} data-soloed={soloed}>
          <div class="channelInfo">
            <IconComp size={15} />
            <span class="channelName">{ch.label}</span>
          </div>
          <div class="faderWell">
            <span class="ticks" aria-hidden="true"><i></i><i></i><i></i><i></i><i></i></span>
            <div class="channelSlider" style:--pct={level}>
              <span class="faderGroove"><span class="faderRange" style:height="{level}%"></span></span>
              <span class="faderCap" style:bottom="calc(var(--pct) * (100% - 38px) / 100)">
                <span class="faderLine"></span>
              </span>
              <input
                class="sliderInput"
                type="range"
                min="0"
                max="100"
                step="1"
                aria-label="{ch.label} level"
                value={doc.current.channels[ch.id]}
                oninput={(event) => dragLevel(ch.id, event.currentTarget)}
                onchange={(event) => commitLevel(ch.id, event.currentTarget)}
              />
            </div>
          </div>
          <div class="channelBottom">
            <span class="channelLevel">{doc.current.channels[ch.id]}</span>
            <div class="padRow">
              <Toggle.Root class="pad" data-kind="mute" pressed={muted} onPressedChange={(pressed) => void doc.change((tx) => setMuted(tx, ch.id, pressed))} aria-label="Mute {ch.label}">M</Toggle.Root>
              <Toggle.Root class="pad" data-kind="solo" pressed={soloed} onPressedChange={(pressed) => void doc.change((tx) => setSoloed(tx, ch.id, pressed))} aria-label="Solo {ch.label}">S</Toggle.Root>
            </div>
          </div>
        </div>
      {/each}
    </section>

    <footer class="footer">
      <div class="vuRow" aria-hidden="true">
        {#each Array(10) as _, index}
          {@const lit = index < vuLevel ? (index >= 8 ? "red" : index >= 6 ? "amber" : "green") : ""}
          <span class="vuSegment" data-lit={lit}></span>
        {/each}
      </div>
      <span class="status" aria-live="polite">{view.status}</span>
    </footer>
  </article>
</main>
