<script lang="ts">
import { ui } from "./ui.svelte";
import doc from "./schema";
</script>

<article class="card" aria-label="Exported recipe for {doc.current.title}">
      <header class="hero" style:grid-template-columns={ui.photoUrl ? undefined : "1fr"}>
        <div class="intro">
          <div class="utilityLine">
            <span class="difficultyTrigger">{doc.current.difficulty}</span>
          </div>
          <h1 class="titleText">{doc.current.title.trim() || "Untitled recipe"}</h1>
          {#if doc.current.description.trim()}<p class="descriptionText">{doc.current.description}</p>{/if}
          <div class="stats">
            <div><span>Serves</span><strong>{doc.current.servings ?? "—"}</strong></div>
            <div><span>Prep</span><span class="number"><strong>{doc.current.prepMinutes ?? "—"}</strong><small>min</small></span></div>
            <div><span>Cook</span><span class="number"><strong>{doc.current.cookMinutes ?? "—"}</strong><small>min</small></span></div>
          </div>
        </div>
        {#if ui.photoUrl}<figure class="photoWell" data-photo="true">
          <img src={ui.photoUrl} alt="" />
        </figure>{/if}
      </header>

      <div class="body">
        <section class="section" aria-labelledby="export-ingredients">
          <div class="sectionTitle"><div><h2 id="export-ingredients">Ingredients</h2></div></div>
          <ul class="list">
            {#each doc.current.ingredients as item (item.$id)}
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
            {#each doc.current.steps as step, index (step.$id)}
              <li class="stepRow">
                <span class="stepNumber">{String(index + 1).padStart(2, "0")}</span>
                <div class="stepCopy">
                  <h3 class="stepTitleText">{step.title.trim() || `Step ${index + 1}`}</h3>
                  {#if step.text.trim()}<p class="stepBodyText">{step.text}</p>{/if}
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
