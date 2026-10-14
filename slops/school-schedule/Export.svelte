<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { DAYS } from "./schema";
  import { DAY_NAMES, activitiesOn, classAt, clockLabel, scheduleView } from "./schedule";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  const data = $derived(snapshot);
  const { periods } = $derived(scheduleView(data));
</script>

<article class={"sch-exportPage"} aria-label="Exported school schedule">
  <header class={"sch-header"}>
    <div class={"sch-identity"}>
      <span class={"sch-eyebrow"}>SCHOOL / SCHEDULE</span>
      <h1 class={"sch-name"}>{data.studentName || "Your week"}</h1>
      <p class={"sch-term"}>{data.term}</p>
    </div>
  </header>
  <div class={"sch-exportDetails"}>
    Homeroom · {data.homeroom || "—"}<br />Locker · {data.locker || "—"}
  </div>
  <div class={"sch-exportDays"}>
    {#each DAYS as day}<section class={"sch-exportDay"}>
        <h2 class={"sch-exportDayTitle"}>{DAY_NAMES[day]}</h2>
        {#each (preview ? (periods).slice(0, 6) : periods) as p}{@const c = classAt(data, day, p.periodKey)}
          <section class={"sch-exportClass"} data-color={c?.color}>
            <p class={"sch-exportMeta"}>
              {p.name} · {clockLabel(p.start)}–{clockLabel(p.end)}
            </p>
            <h3 class={"sch-exportSubject"}>{c?.subject || "Free period"}</h3>
            {#if c}<p class={"sch-exportMeta"}>{c.room}<br />{c.teacher}</p>{/if}
          </section>{/each}
        <h3 class={"sch-heading"}>After class</h3>
        {#each (preview ? (activitiesOn(data, day)).slice(0, 2) : activitiesOn(data, day)) as a}<section
            class={"sch-exportClass"}
          >
            <p class={"sch-exportMeta"}>
              {clockLabel(a.start)}–{clockLabel(a.end)}
            </p>
            <h4 class={"sch-exportSubject"}>{a.title || "Your plans"}</h4>
            <p class={"sch-exportMeta"}>{a.location}</p>
          </section>{:else}<p class={"sch-exportMeta"}>No plans yet.</p>{/each}
      </section>{/each}
  </div>
</article>
