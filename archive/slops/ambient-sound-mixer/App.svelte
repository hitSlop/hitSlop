<script lang="ts">
  import { Slop, bindValue, useDocument } from "@hitslop/document/svelte";
  import { capture } from "@hitslop/document/capture";
  import { onMount, tick, untrack } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { Select, Toggle, Button } from "bits-ui";
  import Power from "@lucide/svelte/icons/power";
  import CloudRain from "@lucide/svelte/icons/cloud-rain";
  import CloudLightning from "@lucide/svelte/icons/cloud-lightning";
  import Wind from "@lucide/svelte/icons/wind";
  import Bird from "@lucide/svelte/icons/bird";
  import Moon from "@lucide/svelte/icons/moon";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Check from "@lucide/svelte/icons/check";
  import schema, { CHANNEL_IDS, type ChannelID, type ChannelLevels } from "./schema";
  import { AmbientEngine } from "./engine";

  type MixerPreset = { name: "Lo-Fi Rain" | "Forest Dawn" | "Midnight Storm" | "Cozy Evening"; levels: ChannelLevels };

  const PRESETS: MixerPreset[] = [
    { name: "Lo-Fi Rain", levels: { rain: 75, thunder: 20, wind: 35, birds: 0, night: 40 } },
    { name: "Forest Dawn", levels: { rain: 15, thunder: 0, wind: 40, birds: 85, night: 10 } },
    { name: "Midnight Storm", levels: { rain: 85, thunder: 70, wind: 60, birds: 0, night: 0 } },
    { name: "Cozy Evening", levels: { rain: 30, thunder: 0, wind: 25, birds: 20, night: 75 } },
  ];
  const PRESET_ITEMS = PRESETS.map((preset) => ({ value: preset.name, label: preset.name }));
  const CHANNELS = [
    { id: "rain" as const, label: "Rain", icon: CloudRain },
    { id: "thunder" as const, label: "Thunder", icon: CloudLightning },
    { id: "wind" as const, label: "Wind", icon: Wind },
    { id: "birds" as const, label: "Birds", icon: Bird },
    { id: "night" as const, label: "Night", icon: Moon },
  ];

  const doc = useDocument(schema);
  const travel = {
    rain: new Tween(untrack(() => doc.current.channels.rain), { duration: 0, easing: cubicOut }),
    thunder: new Tween(untrack(() => doc.current.channels.thunder), { duration: 0, easing: cubicOut }),
    wind: new Tween(untrack(() => doc.current.channels.wind), { duration: 0, easing: cubicOut }),
    birds: new Tween(untrack(() => doc.current.channels.birds), { duration: 0, easing: cubicOut }),
    night: new Tween(untrack(() => doc.current.channels.night), { duration: 0, easing: cubicOut }),
  } satisfies Record<ChannelID, Tween<number>>;

  const engine = new AmbientEngine();
  let audioArmed = false;
  let glide = false;
  let freezeVu = false;
  let vuLevel = $state(0);
  let vuTimer: ReturnType<typeof setInterval> | null = null;

  const anySolo = $derived(CHANNEL_IDS.some((id) => doc.current.soloed[id]));
  const presetItems = $derived(doc.current.preset === "Custom" ? [{ value: "Custom", label: "Custom" }, ...PRESET_ITEMS] : PRESET_ITEMS);
  const mixStatus = $derived.by(() => {
    if (!doc.current.playing) return "Standby";
    if (!CHANNEL_IDS.some((id) => isAudible(id) && doc.current.channels[id] > 0)) return "Silent";
    if (anySolo) return "Solo";
    return "Live";
  });

  function isAudible(id: ChannelID): boolean {
    if (doc.current.muted[id]) return false;
    return anySolo ? doc.current.soloed[id] : true;
  }

  function audibleLevels(): ChannelLevels {
    const next = { ...doc.current.channels };
    for (const id of CHANNEL_IDS) {
      if (!isAudible(id)) next[id] = 0;
    }
    return next;
  }

  function mixEnergy(): number {
    if (!doc.current.playing) return 0;
    const levels = audibleLevels();
    const sum = CHANNEL_IDS.reduce((total, id) => total + levels[id], 0);
    return Math.min(1, (sum / 500) * (doc.current.master / 100));
  }

  function shown(id: ChannelID): number {
    return Math.round(travel[id].current);
  }

  function applyPreset(name: string): void {
    const preset = PRESETS.find((item) => item.name === name);
    if (!preset) return;
    glide = true;
    try {
      doc.change((tx) => {
        tx.fields.preset.set(preset.name);
        for (const id of CHANNEL_IDS) {
          tx.fields.channels[id].set(preset.levels[id]);
          tx.fields.muted[id].set(false);
          tx.fields.soloed[id].set(false);
        }
      });
    } catch (error) {
      glide = false;
      throw error;
    }
  }

  async function togglePower(): Promise<void> {
    const opened = await engine.ensure();
    if (!opened) return;
    if (!audioArmed && doc.current.playing) {
      audioArmed = true;
      engine.setMix(true, doc.current.master, audibleLevels());
      return;
    }
    audioArmed = true;
    doc.fields.playing.set(!doc.current.playing);
  }

  $effect(() => {
    const duration = glide && !prefersReducedMotion.current ? 320 : 0;
    glide = false;
    for (const id of CHANNEL_IDS) void travel[id].set(doc.current.channels[id], { duration, delay: 0 });
  });

  $effect(() => {
    const preset = PRESETS.find((item) => item.name === doc.current.preset);
    if (!preset) return;
    const matches = CHANNEL_IDS.every((id) => Math.round(doc.current.channels[id]) === Math.round(preset.levels[id]));
    if (!matches) doc.fields.preset.set("Custom");
  });

  $effect(() => {
    const playing = doc.current.playing;
    const master = doc.current.master;
    const levels = audibleLevels();
    if (engine.isOpen) engine.setMix(playing, master, levels);
  });

  function paintVu(): void {
    if (freezeVu) return;
    const energy = mixEnergy();
    if (!doc.current.playing || prefersReducedMotion.current) {
      vuLevel = Math.round(energy * 10);
      return;
    }
    vuLevel = Math.max(0, Math.min(10, Math.round(energy * 10 + (Math.random() * 2 - 1))));
  }

  onMount(() => {
    vuTimer = setInterval(paintVu, 120);
    const unregister = capture.onPrepare(async () => {
      freezeVu = true;
      vuLevel = Math.round(mixEnergy() * 10);
      glide = false;
      for (const id of CHANNEL_IDS) await travel[id].set(doc.current.channels[id], { duration: 0, delay: 0 });
      await tick();
    });
    return () => {
      if (vuTimer) clearInterval(vuTimer);
      unregister();
      for (const id of CHANNEL_IDS) void travel[id].set(travel[id].target, { duration: 0, delay: 0 });
      engine.close();
    };
  });
</script>

<Slop>
  <main class="canvas" data-slop-selection="none" aria-label="Ambient sound mixer">
    <article class="chassis">
      <header class="header">
        <div class="brand"><strong>Atmos 01</strong></div>
        <Select.Root type="single" value={doc.current.preset} items={presetItems} onValueChange={(value) => { if (value && value !== "Custom") applyPreset(value); }}>
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
              <input class="sliderInput" type="range" min="0" max="100" step="1" aria-label="Master volume" use:bindValue={doc.fields.master} />
            </div>
          </div>
          <Button.Root type="button" class="power" data-on={doc.current.playing} onclick={() => void togglePower()} aria-pressed={doc.current.playing} aria-label={doc.current.playing ? "Stop mix" : "Start mix"}>
            <Power size={16} />
          </Button.Root>
        </div>
      </header>

      <section class="deck" aria-label="Audio channel faders">
        {#each CHANNELS as ch}
          {@const IconComp = ch.icon}
          {@const level = shown(ch.id)}
          {@const muted = doc.current.muted[ch.id]}
          {@const soloed = doc.current.soloed[ch.id]}
          <div class="strip" data-audible={isAudible(ch.id)} data-soloed={soloed}>
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
                <input class="sliderInput" type="range" min="0" max="100" step="1" aria-label="{ch.label} level" use:bindValue={doc.fields.channels[ch.id]} />
              </div>
            </div>
            <div class="channelBottom">
              <span class="channelLevel">{doc.current.channels[ch.id]}</span>
              <div class="padRow">
                <Toggle.Root class="pad" data-kind="mute" pressed={muted} onPressedChange={(pressed) => doc.fields.muted[ch.id].set(pressed)} aria-label="Mute {ch.label}">M</Toggle.Root>
                <Toggle.Root class="pad" data-kind="solo" pressed={soloed} onPressedChange={(pressed) => doc.fields.soloed[ch.id].set(pressed)} aria-label="Solo {ch.label}">S</Toggle.Root>
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
        <span class="status" aria-live="polite">{mixStatus}</span>
      </footer>
    </article>
  </main>

  {#snippet exportView()}
    {@const data = doc.current}
    {@const solo = CHANNEL_IDS.some((id) => data.soloed[id])}
    {@const live = CHANNEL_IDS.some((id) => {
      if (data.muted[id]) return false;
      const audible = solo ? data.soloed[id] : true;
      return audible && data.channels[id] > 0;
    })}
    {@const statusLabel = !data.playing ? "Standby" : !live ? "Silent" : solo ? "Solo" : "Live"}
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
  {/snippet}

  {#snippet icon()}
    <div class="iconSurface" aria-hidden="true">
      <div class="iconShell">
        <div class="iconTop">
          <span class="iconPill"></span>
          <span class="iconLamp" data-on={doc.current.playing}></span>
        </div>
        <div class="iconDeck">
          {#each CHANNEL_IDS as id}
            <div class="iconSlot">
              <div class="iconCap" style:bottom="{Math.max(8, Math.min(86, doc.current.channels[id]))}%"></div>
            </div>
          {/each}
        </div>
        <div class="iconVu">
          <span></span><span></span><span></span><span></span><span></span><span></span><span data-hot="true"></span>
        </div>
      </div>
    </div>
  {/snippet}
</Slop>
