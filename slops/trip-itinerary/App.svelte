<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { bindText, draft } from "hitslop/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { Tabs, Checkbox, Select, Button, Tooltip } from "bits-ui";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Plane from "@lucide/svelte/icons/plane";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import doc from "./schema";
  import {
    DEFAULT_DAY_SUBTITLE, TAGS, addDay, addPackingItem, addStop, itineraryView, removeDay, removePackingItem,
    removeStop, selectDay, setDayDate, setPackingDone, setStopDone, setStopTime, tagCode, type StopTag,
  } from "./shared";

  const tagItems = TAGS.map((tag) => ({ value: tag.value, label: tag.code }));

  let newTime = $state("10:00");
  let newTitle = $state("");
  let newLocation = $state("");
  let newTag = $state<StopTag>("explore");
  let composer = $state<HTMLInputElement>();
  let packingInput = $state<HTMLInputElement>();
  const newPacking = draft(text => doc.change(tx => addPackingItem(tx, text)));

  const view = $derived(itineraryView(doc.current));
  const flipMs = $derived(prefersReducedMotion.current ? 0 : 220);

  function isTag(value: string): value is StopTag {
    return TAGS.some((tag) => tag.value === value);
  }
  async function submitStop(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const day = view.active;
    const title = newTitle.trim();
    if (!day || !title) return;
    await doc.change(tx => addStop(tx, doc.current, day.dayKey, { time: newTime, title, location: newLocation, tag: newTag }));
    newTitle = "";
    newLocation = "";
    composer?.focus();
  }
  async function submitPacking(event: SubmitEvent): Promise<void> {
    if (await newPacking.submit(event)) packingInput?.focus();
  }
</script>

  <Tooltip.Provider>
    <main class="pass" aria-label="Trip itinerary boarding pass">
      <article class="booklet">
        <div class="main">
          <header class="masthead">
            <div class="mast-top">
              <div class="airline"><Plane size={15} /><span>HITSLOP TRANSIT</span></div>
              <input class="trip-name" use:bindText={doc.fields.tripTitle} aria-label="Trip name" />
            </div>
            <div class="route">
              <div class="city">
                <input class="iata" use:bindText={doc.fields.origin} aria-label="Origin code" maxlength="4" />
                <input class="city-name" use:bindText={doc.fields.originCity} aria-label="Origin city" />
              </div>
              <div class="flight-arrow" aria-hidden="true">
                <span class="flight-line"></span>
                <Plane size={16} />
                <span class="flight-line"></span>
              </div>
              <div class="city city-end">
                <input class="iata" use:bindText={doc.fields.destination} aria-label="Destination code" maxlength="4" />
                <input class="city-name" use:bindText={doc.fields.destCity} aria-label="Destination city" />
              </div>
            </div>
            <div class="stripe">
              <label class="field"><span>PASSENGER</span><input use:bindText={doc.fields.passenger} aria-label="Passenger name" /></label>
              <label class="field"><span>FLIGHT</span><input use:bindText={doc.fields.flight} aria-label="Flight number" /></label>
              <label class="field"><span>GATE</span><input use:bindText={doc.fields.gate} aria-label="Gate" /></label>
              <label class="field"><span>SEAT</span><input use:bindText={doc.fields.seat} aria-label="Seat" /></label>
              <label class="field"><span>PNR</span><input use:bindText={doc.fields.bookingRef} aria-label="Booking reference" /></label>
            </div>
          </header>

          <Tabs.Root value={doc.current.selectedDay} onValueChange={(value) => { if (value) doc.change(tx => selectDay(tx, doc.current, value)); }}>
            <Tabs.List class="day-tabs" aria-label="Trip days">
              {#each view.tabs as tab (tab.day.$id)}
                <Tabs.Trigger value={tab.day.dayKey} class="day-tab">
                  <strong>{tab.day.title}</strong>
                  <span>{tab.total ? `${tab.done}/${tab.total}` : "0"}</span>
                </Tabs.Trigger>
              {/each}
              <Button.Root class="add-day" type="button" data-slop-export="hide" onclick={() => doc.change(tx => addDay(tx, doc.current, crypto.randomUUID(), DEFAULT_DAY_SUBTITLE))} aria-label="Add day">
                <Plus size={14} />
              </Button.Root>
            </Tabs.List>
          </Tabs.Root>

          {#if view.active}
            {@const day = view.active}
            <section class="timeline" aria-label={`Stops for ${day.title}`}>
              <div class="day-head">
                <input class="day-title" use:bindText={doc.at(day).title} aria-label="Day title" />
                <input class="day-date" type="date" value={day.date} onchange={(event) => doc.change(tx => setDayDate(tx, day, event.currentTarget.value))} aria-label="Day date" />
                <input class="day-sub" use:bindText={doc.at(day).subtitle} aria-label="Day theme" />
                {#if doc.current.days.length > 1}
                  <Tooltip.Root>
                    <Tooltip.Trigger class="remove" data-slop-export="hide" aria-label="Remove {day.title}" onclick={() => doc.change(tx => removeDay(tx, doc.current, day))}><Trash2 size={13} /></Tooltip.Trigger>
                    <Tooltip.Portal>
                      <Tooltip.Content class="tooltip" sideOffset={6}>Tear this day from the booklet</Tooltip.Content>
                    </Tooltip.Portal>
                  </Tooltip.Root>
                {/if}
              </div>

              <form class="composer" data-slop-export="hide" onsubmit={submitStop}>
                <input class="time-input" type="time" bind:value={newTime} aria-label="Stop time" />
                <input bind:this={composer} class="comp-title" type="text" bind:value={newTitle} placeholder="Stop title" aria-label="Stop title" required />
                <input class="comp-loc" type="text" bind:value={newLocation} placeholder="Place or note" aria-label="Stop place" />
                <Select.Root type="single" value={newTag} items={tagItems} onValueChange={(value) => { if (isTag(value)) newTag = value; }}>
                  <Select.Trigger class="select-trigger" aria-label="Stop type">
                    <Select.Value placeholder="TAG" />
                    <ChevronDown size={12} strokeWidth={2.2} />
                  </Select.Trigger>
                  <Select.Portal>
                    <Select.Content class="select-content" sideOffset={6} data-slop-export="hide">
                      <Select.Viewport>
                        {#each tagItems as item (item.value)}
                          <Select.Item value={item.value} label={item.label}>
                            {#snippet children({ selected })}
                              {item.label}{#if selected}<Check size={12} strokeWidth={2.4} />{/if}
                            {/snippet}
                          </Select.Item>
                        {/each}
                      </Select.Viewport>
                    </Select.Content>
                  </Select.Portal>
                </Select.Root>
                <Button.Root class="add" type="submit" aria-label="Add stop" disabled={!newTitle.trim()}><Plus size={14} /></Button.Root>
              </form>

              <ul class="stops">
                {#each view.activeStops as event (event.$id)}
                  <li class="stop" data-done={event.done} animate:flip={{ duration: flipMs }}>
                    <div class="time-stub">
                      <Checkbox.Root checked={event.done} onCheckedChange={(checked) => doc.change(tx => setStopDone(tx, doc.current, event.$id, checked === true))} aria-label={event.done ? `Mark ${event.title} incomplete` : `Mark ${event.title} complete`}>
                        {#snippet children({ checked })}{#if checked}<Check size={12} strokeWidth={3.5} />{/if}{/snippet}
                      </Checkbox.Root>
                      <input class="time-input" type="time" value={event.time} onchange={(change) => doc.change(tx => setStopTime(tx, event, change.currentTarget.value))} aria-label="Stop time" />
                    </div>
                    <div class="stop-body">
                      <span class="stamp" data-tag={event.tag}>{tagCode(event.tag)}</span>
                      <div class="stop-copy">
                        <input class="stop-title" use:bindText={doc.at(event).title} aria-label="Stop title" />
                        <input class="stop-loc" use:bindText={doc.at(event).location} aria-label="Stop location" placeholder="Location" />
                      </div>
                    </div>
                    <Tooltip.Root>
                      <Tooltip.Trigger class="remove" data-slop-export="hide" aria-label="Remove {event.title}" onclick={() => doc.change(tx => removeStop(tx, doc.current, event.$id))}><Trash2 size={13} /></Tooltip.Trigger>
                      <Tooltip.Portal>
                        <Tooltip.Content class="tooltip" sideOffset={6}>Remove this stub</Tooltip.Content>
                      </Tooltip.Portal>
                    </Tooltip.Root>
                  </li>
                {:else}
                  <li class="empty">
                    <h2>No stops on this coupon yet.</h2>
                    <p>Add a flight, hotel, or place for {day.title}.</p>
                  </li>
                {/each}
              </ul>
            </section>
          {/if}
        </div>

        <div class="perforation" aria-hidden="true">
          <span class="notch notch-top"></span>
          <span class="notch notch-bottom"></span>
        </div>

        <aside class="stub">
          <div class="stub-head">
            <span class="stub-label">BOARDING STUB</span>
            <strong class="stub-route">{doc.current.origin} ✈ {doc.current.destination}</strong>
          </div>
          <div class="stub-meta">
            <div><span>FLIGHT</span><strong>{doc.current.flight}</strong></div>
            <div><span>GATE</span><strong>{doc.current.gate}</strong></div>
            <div><span>SEAT</span><strong>{doc.current.seat}</strong></div>
          </div>
          <div class="stub-passenger">
            <span>NAME</span>
            <strong>{doc.current.passenger}</strong>
          </div>
          <div class="packing">
            <span class="stub-label">PACKING STUB · {view.packed}/{view.packedTotal}</span>
            <ul class="stub-items">
              {#each doc.current.stubItems as stub (stub.$id)}
                <li class="stub-row" data-done={stub.done}>
                  <Checkbox.Root checked={stub.done} onCheckedChange={(checked) => doc.change(tx => setPackingDone(tx, stub.$id, checked === true))} aria-label={stub.text}>
                    {#snippet children({ checked })}{#if checked}<Check size={10} strokeWidth={3} />{/if}{/snippet}
                  </Checkbox.Root>
                  <input class="stub-text" use:bindText={doc.at(stub).text} aria-label="Checklist item" />
                  <button type="button" class="stub-del" data-slop-export="hide" onclick={() => doc.change(tx => removePackingItem(tx, stub.$id))} aria-label="Remove {stub.text}">×</button>
                </li>
              {:else}
                <li class="empty"><p>Nothing on the stub yet.</p></li>
              {/each}
            </ul>
            <form data-slop-export="hide" onsubmit={submitPacking}>
              <input bind:this={packingInput} class="stub-add" bind:value={newPacking.value} placeholder="Add packing item" aria-label="Add packing item" />
            </form>
          </div>
          <div class="barcode-box" aria-hidden="true">
            <div class="barcode"></div>
            <span class="barcode-num">{doc.current.bookingRef}</span>
          </div>
        </aside>
      </article>
    </main>
  </Tooltip.Provider>
