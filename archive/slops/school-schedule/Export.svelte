<script lang="ts">
  import { useDocument } from "@hitslop/document/svelte";
  import schema, { DAYS } from "./schema";
  import { DAY_NAMES, clockLabel, sortedPeriods } from "./schedule";

  const doc = useDocument(schema);
  const data = $derived(doc.current);
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
        {#each sortedPeriods(data.periods) as p}{@const c = data.classes.find(
            (item) => item.day === day && item.periodKey === p.periodKey,
          )}
          <section class={"sch-exportClass"} data-color={c?.color}>
            <p class={"sch-exportMeta"}>
              {p.name} · {clockLabel(p.start)}–{clockLabel(p.end)}
            </p>
            <h3 class={"sch-exportSubject"}>{c?.subject || "Free period"}</h3>
            {#if c}<p class={"sch-exportMeta"}>{c.room}<br />{c.teacher}</p>{/if}
          </section>{/each}
        <h3 class={"sch-heading"}>After class</h3>
        {#each data.activities
          .filter((a) => a.day === day)
          .sort((a, b) => a.start.localeCompare(b.start)) as a}<section
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
