<script lang="ts">
  import doc from "./schema";

  const totalActive = $derived(doc.current.tasks.filter((task) => !task.done).length);

  const q2Active = $derived(doc.current.tasks.filter((task) => task.zone === "q2" && !task.done).length);

  const q2Ratio = $derived(totalActive > 0 ? Math.round((q2Active / totalActive) * 100) : 0);

  const filled = $derived({
    q1: doc.current.tasks.some((task) => task.zone === "q1" && !task.done),
    q2: doc.current.tasks.some((task) => task.zone === "q2" && !task.done),
    q3: doc.current.tasks.some((task) => task.zone === "q3" && !task.done),
    q4: doc.current.tasks.some((task) => task.zone === "q4" && !task.done),
  });
</script>

<div class="iconSurface" aria-hidden="true">
  <article class="iconSheet">
    <div class="iconHead">
      <span class="iconTitle"></span>
      <span class="iconMeter"><span class="iconMeterFill" style:width="{Math.max(q2Ratio, 12)}%"></span></span>
    </div>
    <div class="iconGrid">
      <span class="iconQuad" data-quad="q1" data-filled={filled.q1}></span>
      <span class="iconQuad" data-quad="q2" data-filled={filled.q2}></span>
      <span class="iconQuad" data-quad="q3" data-filled={filled.q3}></span>
      <span class="iconQuad" data-quad="q4" data-filled={filled.q4}></span>
    </div>
  </article>
</div>
