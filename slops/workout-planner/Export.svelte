<script lang="ts">
  let { preview = false }: { preview?: boolean } = $props();
import doc from "./schema";
import { checked, sets, workoutView } from "./workout";
const { total, done, ratio } = $derived(workoutView(doc.current));
</script>

<article class="export-board" aria-label="Workout summary">
      <span class="eyebrow">Training / {done} of {total} sets</span>
      <h1 class="exercise-name">{doc.current.title || "Your workout"}</h1>
      <div class="progress">
        <div class="fill" style:width={`${ratio}%`}></div>
      </div>
      {#each (preview ? (doc.current.exercises).slice(0, 4) : doc.current.exercises) as ex}
        <section class="export-lift">
          <h2 class="export-title">{ex.name}</h2>
          <p class="description">{sets(ex)} sets × {ex.reps} reps · {ex.weight || "Bodyweight"}</p>
          <div class="ticks">
            {#each Array.from({ length: sets(ex) }, (_, index) => index) as index}
              <span class="tick" data-state={checked(ex).includes(index) ? "checked" : "unchecked"} aria-label="Set {index + 1}: {checked(ex).includes(index) ? 'complete' : 'unfinished'}">{checked(ex).includes(index) ? "✓" : index + 1}</span>
            {/each}
          </div>
        </section>
      {:else}
        <p class="description">No exercises yet.</p>
      {/each}
    </article>
