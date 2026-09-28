<script lang="ts">
  import { Slop, bindText, bindValue, useDocument } from "@hitslop/document/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { Tabs, Checkbox, Select, Button, Tooltip } from "bits-ui";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Plane from "@lucide/svelte/icons/plane";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import schema, { stopTags, type Day, type Stop, type StopTag } from "./schema";

  const TAGS = [
    { value: "flight", label: "Flight", code: "FLT" },
    { value: "hotel", label: "Hotel", code: "HTL" },
    { value: "dining", label: "Dining", code: "DINE" },
    { value: "train", label: "Transit", code: "RAIL" },
    { value: "explore", label: "Explore", code: "ACT" },
  ] as const;
  const tagItems = TAGS.map((tag) => ({ value: tag.value, label: tag.code }));

  const doc = useDocument(schema);

  let newTime = $state("10:00");
  let newTitle = $state("");
  let newLocation = $state("");
  let newTag = $state<StopTag>("explore");
  let newStubText = $state("");
  let composer = $state<HTMLInputElement>();

  const orderedDays = $derived(orderDays(doc.current.days));
  const activeDay = $derived(doc.current.days.find((day) => day.dayKey === doc.current.selectedDay) ?? doc.current.days[0]);
  const orderedStops = $derived(activeDay ? orderStops(activeDay.events) : []);
  const packedCount = $derived(doc.current.stubItems.filter((item) => item.done).length);
  const packedTotal = $derived(doc.current.stubItems.length);
  const flipMs = $derived(prefersReducedMotion.current ? 0 : 220);

  function orderDays(days: readonly Day[]): Day[] {
    return days
      .map((day, index) => ({ day, index }))
      .sort((a, b) => {
        if (a.day.date && b.day.date && a.day.date !== b.day.date) return a.day.date.localeCompare(b.day.date);
        if (a.day.date && !b.day.date) return -1;
        if (!a.day.date && b.day.date) return 1;
        return a.index - b.index;
      })
      .map((item) => item.day);
  }
  function orderStops(events: readonly Stop[]): Stop[] {
    return events
      .map((event, index) => ({ event, index }))
      .sort((a, b) => {
        if (a.event.time !== b.event.time) return (a.event.time || "").localeCompare(b.event.time || "");
        return a.index - b.index;
      })
      .map((item) => item.event);
  }
  function isTag(value: string): value is StopTag {
    return stopTags.some((tag) => tag === value);
  }
  function tagCode(id: StopTag): string {
    return TAGS.find((tag) => tag.value === id)?.code ?? id.toUpperCase();
  }
  function nextDate(from: string): string {
    const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(from);
    if (!match) return "";
    const date = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]) + 1);
    return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
  }
  function formatDate(iso: string): string {
    const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
    if (!match) return iso;
    return new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3])).toLocaleDateString(undefined, { month: "short", day: "numeric" });
  }
  function addEvent(): void {
    const title = newTitle.trim();
    if (!activeDay || !title) return;
    doc.at(activeDay).events.insert({
      time: newTime || "10:00",
      title,
      location: newLocation.trim(),
      tag: newTag,
      done: false,
    });
    newTitle = "";
    newLocation = "";
    composer?.focus();
  }
  function removeEvent(id: string): void {
    if (!activeDay) return;
    doc.at(activeDay).events.remove(id);
  }
  function addDay(): void {
    const last = orderedDays[orderedDays.length - 1];
    const dayKey = crypto.randomUUID();
    doc.change((tx) => {
      tx.fields.days.insert({
        dayKey,
        title: `DAY ${String(doc.current.days.length + 1).padStart(2, "0")}`,
        subtitle: "Exploration & Activities",
        date: last ? nextDate(last.date) : "",
        events: [],
      });
      tx.fields.selectedDay.set(dayKey);
    });
  }
  function removeDay(day: Day): void {
    if (doc.current.days.length <= 1) return;
    const next = doc.current.days.find((entry) => entry.$id !== day.$id);
    doc.change((tx) => {
      tx.fields.days.remove(day.$id);
      if (doc.current.selectedDay === day.dayKey && next) tx.fields.selectedDay.set(next.dayKey);
    });
  }
  function addStubItem(): void {
    const text = newStubText.trim();
    if (!text) return;
    doc.fields.stubItems.insert({ text, done: false });
    newStubText = "";
  }
  function removeStubItem(id: string): void {
    doc.fields.stubItems.remove(id);
  }
  function dayCounts(day: Day): { done: number; total: number } {
    return { total: day.events.length, done: day.events.filter((event) => event.done).length };
  }
  function selectDay(value: string) {
    if (doc.current.days.some((day) => day.dayKey === value)) doc.fields.selectedDay.set(value);
  }
</script>

<Slop>
  <Tooltip.Provider>
    <main class="pass" data-slop-selection="none" aria-label="Trip itinerary boarding pass">
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

          <Tabs.Root value={doc.current.selectedDay} onValueChange={(value) => { if (value) selectDay(value); }}>
            <Tabs.List class="day-tabs" aria-label="Trip days">
              {#each orderedDays as day (day.$id)}
                {@const counts = dayCounts(day)}
                <Tabs.Trigger value={day.dayKey} class="day-tab">
                  <strong>{day.title}</strong>
                  <span>{counts.total ? `${counts.done}/${counts.total}` : "0"}</span>
                </Tabs.Trigger>
              {/each}
              <Button.Root class="add-day" type="button" data-slop-export="hide" onclick={addDay} aria-label="Add day">
                <Plus size={14} />
              </Button.Root>
            </Tabs.List>
          </Tabs.Root>

          {#if activeDay}
            <section class="timeline" aria-label={`Stops for ${activeDay.title}`}>
              <div class="day-head">
                <input class="day-title" use:bindText={doc.at(activeDay).title} aria-label="Day title" />
                <input class="day-date" type="date" use:bindValue={doc.at(activeDay).date} aria-label="Day date" />
                <input class="day-sub" use:bindText={doc.at(activeDay).subtitle} aria-label="Day theme" />
                {#if doc.current.days.length > 1}
                  <Tooltip.Root>
                    <Tooltip.Trigger class="remove" data-slop-export="hide" aria-label="Remove {activeDay.title}" onclick={() => activeDay && removeDay(activeDay)}><Trash2 size={13} /></Tooltip.Trigger>
                    <Tooltip.Portal>
                      <Tooltip.Content class="tooltip" sideOffset={6}>Tear this day from the booklet</Tooltip.Content>
                    </Tooltip.Portal>
                  </Tooltip.Root>
                {/if}
              </div>

              <form class="composer" data-slop-export="hide" onsubmit={(event) => { event.preventDefault(); addEvent(); }}>
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
                {#each orderedStops as event (event.$id)}
                  <li class="stop" data-done={event.done} animate:flip={{ duration: flipMs }}>
                    <div class="time-stub">
                      <Checkbox.Root checked={event.done} onCheckedChange={(checked) => doc.at(event).done.set(checked === true)} aria-label={event.done ? `Mark ${event.title} incomplete` : `Mark ${event.title} complete`}>
                        {#snippet children({ checked })}{#if checked}<Check size={12} strokeWidth={3.5} />{/if}{/snippet}
                      </Checkbox.Root>
                      <input class="time-input" type="time" use:bindValue={doc.at(event).time} aria-label="Stop time" />
                    </div>
                    <div class="stop-body">
                      <span class="stamp" data-tag={event.tag}>{tagCode(event.tag)}</span>
                      <div class="stop-copy">
                        <input class="stop-title" use:bindText={doc.at(event).title} aria-label="Stop title" />
                        <input class="stop-loc" use:bindText={doc.at(event).location} aria-label="Stop location" placeholder="Location" />
                      </div>
                    </div>
                    <Tooltip.Root>
                      <Tooltip.Trigger class="remove" data-slop-export="hide" aria-label="Remove {event.title}" onclick={() => removeEvent(event.$id)}><Trash2 size={13} /></Tooltip.Trigger>
                      <Tooltip.Portal>
                        <Tooltip.Content class="tooltip" sideOffset={6}>Remove this stub</Tooltip.Content>
                      </Tooltip.Portal>
                    </Tooltip.Root>
                  </li>
                {:else}
                  <li class="empty">
                    <h2>No stops on this coupon yet.</h2>
                    <p>Add a flight, hotel, or place for {activeDay.title}.</p>
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
            <span class="stub-label">PACKING STUB · {packedCount}/{packedTotal}</span>
            <ul class="stub-items">
              {#each doc.current.stubItems as stub (stub.$id)}
                <li class="stub-row" data-done={stub.done}>
                  <Checkbox.Root checked={stub.done} onCheckedChange={(checked) => doc.at(stub).done.set(checked === true)} aria-label={stub.text}>
                    {#snippet children({ checked })}{#if checked}<Check size={10} strokeWidth={3} />{/if}{/snippet}
                  </Checkbox.Root>
                  <input class="stub-text" use:bindText={doc.at(stub).text} aria-label="Checklist item" />
                  <button type="button" class="stub-del" data-slop-export="hide" onclick={() => removeStubItem(stub.$id)} aria-label="Remove {stub.text}">×</button>
                </li>
              {:else}
                <li class="empty"><p>Nothing on the stub yet.</p></li>
              {/each}
            </ul>
            <form data-slop-export="hide" onsubmit={(event) => { event.preventDefault(); addStubItem(); }}>
              <input class="stub-add" bind:value={newStubText} placeholder="Add packing item" aria-label="Add packing item" />
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

  {#snippet exportView()}
    {@const days = orderDays(doc.current.days)}
    {@const packed = doc.current.stubItems.filter((item) => item.done).length}
    <article class="export-pass" aria-label="Exported trip itinerary">
      <div class="export-main">
        <header class="masthead">
          <div class="mast-top">
            <div class="airline"><Plane size={15} /><span>HITSLOP TRANSIT</span></div>
            <strong class="trip-name">{doc.current.tripTitle || "Untitled trip"}</strong>
          </div>
          <div class="route">
            <div class="city">
              <strong class="iata">{doc.current.origin}</strong>
              <span class="city-name">{doc.current.originCity}</span>
            </div>
            <div class="flight-arrow" aria-hidden="true">
              <span class="flight-line"></span>
              <Plane size={16} />
              <span class="flight-line"></span>
            </div>
            <div class="city city-end">
              <strong class="iata">{doc.current.destination}</strong>
              <span class="city-name">{doc.current.destCity}</span>
            </div>
          </div>
          <div class="stripe">
            <div class="field"><span>PASSENGER</span><strong>{doc.current.passenger}</strong></div>
            <div class="field"><span>FLIGHT</span><strong>{doc.current.flight}</strong></div>
            <div class="field"><span>GATE</span><strong>{doc.current.gate}</strong></div>
            <div class="field"><span>SEAT</span><strong>{doc.current.seat}</strong></div>
            <div class="field"><span>PNR</span><strong>{doc.current.bookingRef}</strong></div>
          </div>
        </header>

        {#each days as day (day.$id)}
          {@const stops = orderStops(day.events)}
          <section class="export-day" aria-label={day.title}>
            <div class="export-day-head">
              <strong>{day.title}</strong>
              {#if day.date}<span>{formatDate(day.date)}</span>{/if}
              <span>{day.subtitle}</span>
            </div>
            <ul class="stops">
              {#each stops as event (event.$id)}
                <li class="stop" data-done={event.done}>
                  <div class="time-stub">
                    <span data-checkbox-root data-state={event.done ? "checked" : "unchecked"}>{#if event.done}<Check size={12} strokeWidth={3.5} />{/if}</span>
                    <span class="time-input">{event.time}</span>
                  </div>
                  <div class="stop-body">
                    <span class="stamp" data-tag={event.tag}>{tagCode(event.tag)}</span>
                    <div class="stop-copy">
                      <span class="stop-title">{event.title || "Untitled stop"}</span>
                      <span class="stop-loc">{event.location}</span>
                    </div>
                  </div>
                </li>
              {:else}
                <li class="empty"><p>No stops on this day.</p></li>
              {/each}
            </ul>
          </section>
        {:else}
          <div class="empty"><h2>No days in this booklet yet.</h2></div>
        {/each}
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
          <span class="stub-label">PACKING STUB · {packed}/{doc.current.stubItems.length}</span>
          <ul class="stub-items">
            {#each doc.current.stubItems as stub (stub.$id)}
              <li class="stub-row" data-done={stub.done}>
                <span data-checkbox-root data-state={stub.done ? "checked" : "unchecked"}>{#if stub.done}<Check size={10} strokeWidth={3} />{/if}</span>
                <span class="stub-text">{stub.text || "Untitled item"}</span>
              </li>
            {:else}
              <li class="empty"><p>Nothing on the stub yet.</p></li>
            {/each}
          </ul>
        </div>
        <div class="barcode-box" aria-hidden="true">
          <div class="barcode"></div>
          <span class="barcode-num">{doc.current.bookingRef}</span>
        </div>
      </aside>
    </article>
  {/snippet}

  {#snippet icon()}
    <div class="icon-surface" aria-hidden="true">
      <article class="icon-pass">
        <div class="icon-main">
          <div class="icon-head">
            <span class="icon-airline">HITSLOP AIRWAYS</span>
            <span class="icon-class">BOARDING</span>
          </div>
          <div class="icon-route">
            <div class="icon-city"><strong>{doc.current.origin || "SFO"}</strong><span>{doc.current.originCity || "ORIGIN"}</span></div>
            <span class="icon-plane">✈</span>
            <div class="icon-city"><strong>{doc.current.destination || "NRT"}</strong><span>{doc.current.destCity || "DEST"}</span></div>
          </div>
          <div class="icon-meta">
            <div><span>FLIGHT</span><strong>{doc.current.flight || "—"}</strong></div>
            <div><span>GATE</span><strong>{doc.current.gate || "—"}</strong></div>
            <div><span>SEAT</span><strong>{doc.current.seat || "—"}</strong></div>
          </div>
        </div>
        <div class="icon-perf">
          <span class="icon-notch icon-notch-top"></span>
          <span class="icon-notch icon-notch-bottom"></span>
        </div>
        <div class="icon-stub">
          <div class="icon-barcode"></div>
          <span class="icon-code">{packedTotal > 0 && packedCount === packedTotal ? "BOARDED" : "OPEN"}</span>
        </div>
      </article>
    </div>
  {/snippet}
</Slop>
