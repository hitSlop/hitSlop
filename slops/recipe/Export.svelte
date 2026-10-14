<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import { previewText } from "../_shared/preview";
  import type schema from "./schema";
  import { recipeView, stepTitle } from "./recipe";
  let { snapshot, attachmentURL, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  const view = $derived(recipeView(snapshot));
  // The export is its own page: it reads the saved photo itself rather than sharing the editor's state.
  const photoURL = $derived(snapshot.photo ? attachmentURL(snapshot.photo.id) : "");
</script>

<article class="card" aria-label="Exported recipe for {snapshot.title}">
      <header class="hero" style:grid-template-columns={photoURL ? undefined : "1fr"}>
        <div class="intro">
          <div class="utilityLine">
            <span class="difficultyTrigger">{snapshot.difficulty}</span>
          </div>
          <h1 class="titleText">{view.title}</h1>
          {#if snapshot.description.trim()}<p class="descriptionText">{previewText(snapshot.description, preview, 160)}</p>{/if}
          <div class="stats">
            <div><span>Serves</span><strong>{snapshot.servings ?? "—"}</strong></div>
            <div><span>Prep</span><span class="number"><strong>{snapshot.prepMinutes ?? "—"}</strong><small>min</small></span></div>
            <div><span>Cook</span><span class="number"><strong>{snapshot.cookMinutes ?? "—"}</strong><small>min</small></span></div>
          </div>
        </div>
        {#if photoURL}<figure class="photoWell" data-photo="true">
          <img src={photoURL} alt="" />
        </figure>{/if}
      </header>

      <div class="body">
        <section class="section" aria-labelledby="export-ingredients">
          <div class="sectionTitle"><div><h2 id="export-ingredients">Ingredients</h2></div></div>
          <ul class="list">
            {#each (preview ? (snapshot.ingredients).slice(0, 6) : snapshot.ingredients) as item (item.$id)}
              <li class="ingredientRow" data-checked={item.checked}>
                <span data-checkbox-root data-state={item.checked ? "checked" : "unchecked"}></span>
                <span class="itemText">{item.text.trim() || "Untitled ingredient"}</span>
              </li>
            {:else}
              <li class="empty">No ingredients yet.</li>
            {/each}
          </ul>
        </section>
        <section class="section" aria-labelledby="export-method">
          <div class="sectionTitle"><div><h2 id="export-method">Method</h2></div></div>
          <ol class="list">
            {#each (preview ? (snapshot.steps).slice(0, 2) : snapshot.steps) as step, index (step.$id)}
              <li class="stepRow">
                <span class="stepNumber">{String(index + 1).padStart(2, "0")}</span>
                <div class="stepCopy">
                  <h3 class="stepTitleText">{stepTitle(step, index)}</h3>
                  {#if step.text.trim()}<p class="stepBodyText">{previewText(step.text, preview, 180)}</p>{/if}
                </div>
                {#if step.minutes}<span class="stepTime">{step.minutes} min</span>{/if}
              </li>
            {:else}
              <li class="empty">No steps yet.</li>
            {/each}
          </ol>
        </section>
      </div>
    </article>
