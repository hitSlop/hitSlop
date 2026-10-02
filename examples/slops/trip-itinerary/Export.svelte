<script lang="ts">
  import doc from "./schema";
  import Check from "@lucide/svelte/icons/check";
  import Plane from "@lucide/svelte/icons/plane";
  import { TAGS, formatDate, orderDays, orderStops, tagCode } from "./shared";

  const days = $derived(orderDays(doc.current.days));
  const packed = $derived(doc.current.stubItems.filter((item) => item.done).length);
</script>

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
