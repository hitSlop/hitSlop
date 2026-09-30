<div class="iconSurface" aria-hidden="true">
      <article class="iconPage">
        <div class="iconGrid">
          {#each { length: 9 } as _, index (index)}
            <span class="iconCell" data-role={index === 4 ? "goal" : "theme"}></span>
          {/each}
        </div>
      </article>
    </div>
