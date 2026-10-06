<script lang="ts">
  import { ui } from "./ui.svelte";
  import doc from "./schema";
  import Mail from "@lucide/svelte/icons/mail";
  import Phone from "@lucide/svelte/icons/phone";
  import Globe from "@lucide/svelte/icons/globe";
  import MapPin from "@lucide/svelte/icons/map-pin";
  import { hrefForWebsite, indexLetter, telHref } from "./shared";
  import { initialsOf } from "./shared";

  const data = $derived(doc.current);
  const hasChannel = $derived(Boolean(data.email.trim() || data.phone.trim() || data.website.trim()));

  const tabLetter = $derived(indexLetter(doc.current.name));

  const websiteHref = $derived(hrefForWebsite(doc.current.website));

  const phoneHref = $derived(telHref(doc.current.phone));

  const mailHref = $derived(doc.current.email.trim() ? `mailto:${doc.current.email.trim()}` : "");

  const monogram = $derived(initialsOf(doc.current.name));
</script>

<article class="exportCanvas" aria-label="Exported contact card">
  <span class="indexTab" aria-hidden="true">{tabLetter}</span>
  <section class="badge">
    <div class="notch" aria-hidden="true"></div>
    <p class="stamp">PAGER 01</p>
    <div class="hero">
      <div class="avatar" aria-hidden="true">{@render portrait(false)}</div>
      <div class="profile">
        <h1 class="nameText">{data.name.trim() || "Unnamed contact"}</h1>
        {#if data.headline.trim()}<p class="headlineText">{data.headline}</p>{/if}
        {#if data.location.trim()}
          <div class="locationTag"><MapPin size={12} /><span class="locationText">{data.location}</span></div>
        {/if}
      </div>
    </div>
    {#if data.bio.trim()}
      <section class="bioBox"><p class="bioText">{data.bio}</p></section>
    {/if}
    <ul class="channels" aria-label="Contact channels">
      {#if data.email.trim()}
        <li class="row"><div class="bubble"><Mail size={14} /></div><div class="fields"><span class="label">Email</span><span class="valueText">{data.email}</span></div></li>
      {/if}
      {#if data.phone.trim()}
        <li class="row"><div class="bubble"><Phone size={14} /></div><div class="fields"><span class="label">Phone</span><span class="valueText">{data.phone}</span></div></li>
      {/if}
      {#if data.website.trim()}
        <li class="row"><div class="bubble"><Globe size={14} /></div><div class="fields"><span class="label">Website</span><span class="valueText">{data.website}</span></div></li>
      {/if}
      {#if !hasChannel}
        <li class="empty">No contact channels yet.</li>
      {/if}
    </ul>
    <footer class="footer">
      <div class="circles">
        <a class="circleBtn" href={mailHref || undefined} aria-label="Compose email" aria-disabled={!mailHref}><Mail size={16} /></a>
        <a class="circleBtn" href={phoneHref || undefined} aria-label="Call" aria-disabled={!phoneHref}><Phone size={16} /></a>
        <a class="circleBtn" href={websiteHref || undefined} target="_blank" rel="noopener noreferrer" aria-label="Visit website" aria-disabled={!websiteHref}><Globe size={16} /></a>
      </div>
    </footer>
  </section>
</article>

{#snippet portrait(editable: boolean)}
  {#if ui.avatarSrc}
    <img class="avatarImg" src={ui.avatarSrc} alt="" />
  {:else if monogram}
    <span class="monogram">{monogram}</span>
  {:else}
    <svg class="avatarImg" viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <rect width="100" height="100" fill="var(--slop-avatarFill)" />
      <circle cx="50" cy="38" r="18" fill="var(--slop-avatarInk)" />
      <path d="M22 84C22 68 34 58 50 58C66 58 78 68 78 84" fill="var(--slop-avatarInk)" />
    </svg>
  {/if}

{/snippet}
