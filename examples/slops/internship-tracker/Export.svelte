<script lang="ts">
  import doc, { stages } from "./schema";
  import { dueLabel, shortDate } from "./dates";
  import { stageCounts, needsNudge } from "./pipeline";
  import { ui } from "./ui.svelte";

  const counts = $derived(stageCounts(doc.current.applications));
  const nudges = $derived(doc.current.applications.filter(needsNudge).length);
</script>

<article class="badge export">
  <header class="band"><p>Hello, I’m applying to</p></header>
  <div class="title-block">
    <h1 class="title">{doc.current.title || "Summer internships"}</h1>
    <p class="nudge" data-hot={nudges > 0}>{nudges > 0 ? `${nudges} follow-up${nudges === 1 ? "" : "s"} due` : "No follow-ups due"}</p>
  </div>
  <div class="tally" aria-hidden="true">
    {#each stages as stage}<span class="seg" data-stage={stage} style:flex-grow={counts[stage]} hidden={counts[stage] === 0}></span>{/each}
  </div>
  <ul class="legend">
    {#each stages as stage}<li data-stage={stage}><b>{counts[stage]}</b> {stage}</li>{/each}
  </ul>
  {#if ui.tab === "applications"}
    <ul class="rows">
      {#each doc.current.applications as app (app.$id)}
        {@const due = dueLabel(app.followUp)}
        <li class="row" data-stage={app.stage}>
          <span class="stamp">{app.stage}</span>
          <div class="who"><span class="company">{app.company || "Untitled"}</span><span class="role">{app.role}</span></div>
          <div class="follow" data-late={due?.late && app.stage !== "offer" && app.stage !== "rejected" ? "" : undefined}>
            {#if app.followUp}<span>{shortDate(app.followUp)}</span>{#if app.stage !== "offer" && app.stage !== "rejected"}<span>{due?.text}</span>{/if}{/if}
          </div>
        </li>
      {:else}
        <li class="empty"><h2>No applications yet.</h2></li>
      {/each}
    </ul>
  {:else}
    <ul class="rows">
      {#each doc.current.chats as chat (chat.$id)}
        <li class="row chat">
          <span class="thanks" data-checked={chat.thanked}>{chat.thanked ? "✓" : ""}</span>
          <div class="who"><span class="company">{chat.name || "Untitled"}</span><span class="role">{chat.place}</span></div>
          <div class="follow"><span>{shortDate(chat.when)}</span><span>{chat.thanked ? "thanked" : "thank-you due"}</span></div>
        </li>
      {:else}
        <li class="empty"><h2>No coffee chats yet.</h2></li>
      {/each}
    </ul>
  {/if}
</article>
