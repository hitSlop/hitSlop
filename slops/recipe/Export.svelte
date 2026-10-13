<script lang="ts">
  import { previewText } from "../_shared/preview";
  let { preview = false }: { preview?: boolean } = $props();
import { attachments } from "hitslop/svelte";
import doc from "./schema";
import { recipeView, stepTitle } from "./recipe";

const view = $derived(recipeView(doc.current));
// The export is its own page: it reads the saved photo itself rather than sharing the editor's state.
const photoURL = $derived(doc.current.photo ? attachments.url(doc.current.photo.id) : "");
</script>

<article class="card" aria-label="Exported recipe for {doc.current.title}">
      <header class="hero" style:grid-template-columns={photoURL ? undefined : "1fr"}>
        <div class="intro">
          <div class="utilityLine">
            <span class="difficultyTrigger">{doc.current.difficulty}</span>
          </div>
          <h1 class="titleText">{view.title}</h1>
          {#if doc.current.description.trim()}<p class="descriptionText">{previewText(doc.current.description, preview, 160)}</p>{/if}
          <div class="stats">
            <div><span>Serves</span><strong>{doc.current.servings ?? "—"}</strong></div>
            <div><span>Prep</span><span class="number"><strong>{doc.current.prepMinutes ?? "—"}</strong><small>min</small></span></div>
            <div><span>Cook</span><span class="number"><strong>{doc.current.cookMinutes ?? "—"}</strong><small>min</small></span></div>
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
            {#each (preview ? (doc.current.ingredients).slice(0, 6) : doc.current.ingredients) as item (item.$id)}
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
            {#each (preview ? (doc.current.steps).slice(0, 2) : doc.current.steps) as step, index (step.$id)}
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
