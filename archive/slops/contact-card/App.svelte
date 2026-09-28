<script lang="ts">
  import { onMount } from "svelte";
  import { Button, Tooltip } from "bits-ui";
  import Mail from "@lucide/svelte/icons/mail";
  import Phone from "@lucide/svelte/icons/phone";
  import Globe from "@lucide/svelte/icons/globe";
  import MapPin from "@lucide/svelte/icons/map-pin";
  import Camera from "@lucide/svelte/icons/camera";
  import Copy from "@lucide/svelte/icons/copy";
  import Check from "@lucide/svelte/icons/check";
  import Download from "@lucide/svelte/icons/download";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import { attachments } from "@hitslop/document/attachments";
  import { capture } from "@hitslop/document/capture";
  import { Slop, bindText, bindValue, useDocument } from "@hitslop/document/svelte";
  import schema from "./schema";

  const doc = useDocument(schema);
  let copiedField = $state<string | null>(null);
  let fileInput = $state<HTMLInputElement>();
  let avatarSrc = $state("");
  let portraitTask: Promise<void> = Promise.resolve();

  const tabLetter = $derived(indexLetter(doc.current.name));
  const monogram = $derived(initialsOf(doc.current.name));
  const websiteHref = $derived(hrefForWebsite(doc.current.website));
  const phoneHref = $derived(telHref(doc.current.phone));
  const mailHref = $derived(doc.current.email.trim() ? `mailto:${doc.current.email.trim()}` : "");
  const portraitId = $derived(doc.current.avatar?.id ?? "");
  const portraitType = $derived(doc.current.avatar?.mimeType ?? "");

  $effect(() => {
    if (!copiedField) return;
    const field = copiedField;
    const timer = window.setTimeout(() => { if (copiedField === field) copiedField = null; }, 1800);
    return () => window.clearTimeout(timer);
  });
  $effect(() => {
    const id = portraitId;
    const type = portraitType;
    let cancelled = false;
    let url = "";
    portraitTask = (async () => {
      if (!id) {
        avatarSrc = "";
        return;
      }
      try {
        const blob = await attachments.read(id, { type });
        const next = URL.createObjectURL(blob);
        url = next;
        if (cancelled) {
          URL.revokeObjectURL(next);
          return;
        }
        avatarSrc = next;
      } catch {
        if (!cancelled) avatarSrc = "";
      }
    })();
    return () => {
      cancelled = true;
      if (url) URL.revokeObjectURL(url);
    };
  });
  onMount(() => capture.onPrepare(() => portraitTask));

  function indexLetter(name: string): string {
    return (name.trim()[0] ?? "?").toUpperCase();
  }
  function initialsOf(name: string): string {
    const parts = name.trim().split(/\s+/).filter(Boolean);
    if (!parts.length) return "";
    const first = parts[0] ?? "";
    const last = parts.length > 1 ? parts[parts.length - 1] ?? "" : "";
    return last ? `${first[0] ?? ""}${last[0] ?? ""}`.toUpperCase() : first.slice(0, 2).toUpperCase();
  }
  function hrefForWebsite(url: string): string {
    const value = url.trim();
    if (!value) return "";
    return /^https?:\/\//i.test(value) ? value : `https://${value}`;
  }
  function telHref(phone: string): string {
    const value = phone.trim();
    if (!value) return "";
    return `tel:${value.replace(/[^\d+]/g, "")}`;
  }
  function vCardEscape(value: string): string {
    return value.replace(/\\/g, "\\\\").replace(/\n/g, "\\n").replace(/,/g, "\\,").replace(/;/g, "\\;");
  }
  async function copyToClipboard(text: string, fieldId: string) {
    const value = text.trim();
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
      copiedField = fieldId;
    } catch {
      copiedField = null;
    }
  }
  function downloadVCard() {
    const data = doc.current;
    const lines = ["BEGIN:VCARD", "VERSION:3.0", `FN:${vCardEscape(data.name.trim() || "Unnamed contact")}`];
    if (data.headline.trim()) lines.push(`TITLE:${vCardEscape(data.headline)}`);
    if (data.email.trim()) lines.push(`EMAIL;TYPE=INTERNET,WORK:${vCardEscape(data.email)}`);
    if (data.phone.trim()) lines.push(`TEL;TYPE=CELL:${vCardEscape(data.phone)}`);
    if (data.website.trim()) lines.push(`URL:${vCardEscape(hrefForWebsite(data.website) || data.website)}`);
    if (data.location.trim()) lines.push(`ADR;TYPE=WORK:;${vCardEscape(data.location)};;`);
    if (data.bio.trim()) lines.push(`NOTE:${vCardEscape(data.bio)}`);
    lines.push("END:VCARD");
    const blob = new Blob([lines.join("\r\n")], { type: "text/vcard;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `${(data.name.trim() || "contact").toLowerCase().replace(/\s+/g, "_")}.vcf`;
    link.click();
    URL.revokeObjectURL(url);
  }
  function onPortrait(event: Event) {
    const input = event.currentTarget;
    if (!(input instanceof HTMLInputElement)) return;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    void attachments.import(file, { commit(ref) {
      doc.fields.avatar.set({ id: ref.id, mimeType: ref.mimeType });
    } });
  }
</script>

{#snippet portrait(editable: boolean)}
  {#if avatarSrc}
    <img class="avatarImg" src={avatarSrc} alt="" />
  {:else if monogram}
    <span class="monogram">{monogram}</span>
  {:else}
    <svg class="avatarImg" viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <rect width="100" height="100" fill="var(--slop-avatarFill)" />
      <circle cx="50" cy="38" r="18" fill="var(--slop-avatarInk)" />
      <path d="M22 84C22 68 34 58 50 58C66 58 78 68 78 84" fill="var(--slop-avatarInk)" />
    </svg>
  {/if}
  {#if editable}
    <span class="overlay" data-slop-export="hide"><Camera size={20} /></span>
  {/if}
{/snippet}

<Slop>
  <Tooltip.Provider delayDuration={200}>
    <main class="canvas" data-slop-selection="none" aria-label="Contact card">
      <input hidden type="file" accept="image/*" bind:this={fileInput} data-slop-export="hide" onchange={onPortrait} />
      <div class="tray">
        <span class="indexTab" aria-hidden="true">{tabLetter}</span>
        <article class="badge">
          <div class="notch" aria-hidden="true"></div>
          <p class="stamp">PAGER 01</p>

          <section class="hero" aria-label="Profile overview">
            <Button.Root type="button" class="avatar" onclick={() => fileInput?.click()} aria-label="Change profile photo">
              {@render portrait(true)}
            </Button.Root>
            <div class="profile">
              <input class="nameInput" aria-label="Full name" placeholder="Your name" use:bindText={doc.fields.name} />
              <input class="headlineInput" aria-label="Professional headline" placeholder="Title or role" use:bindText={doc.fields.headline} />
              <div class="locationTag">
                <MapPin size={12} />
                <input class="locInput" aria-label="Location" placeholder="City" use:bindText={doc.fields.location} />
              </div>
            </div>
          </section>

          <section class="bioBox" aria-label="Short biography">
            <textarea class="bioInput" rows="3" aria-label="Short bio" placeholder="A short note about you." use:bindText={doc.fields.bio}></textarea>
          </section>

          <ul class="channels" aria-label="Contact channels">
            <li class="row">
              <div class="bubble"><Mail size={14} /></div>
              <div class="fields">
                <span class="label">Email</span>
                <input class="valueInput" aria-label="Email address" placeholder="name@example.com" use:bindValue={doc.fields.email} />
              </div>
              <Tooltip.Root disableCloseOnTriggerClick>
                <Tooltip.Trigger class="actionBtn" data-slop-export="hide" aria-label={copiedField === "email" ? "Email copied" : "Copy email"} disabled={!doc.current.email.trim()} onclick={() => copyToClipboard(doc.current.email, "email")}>
                  {#if copiedField === "email"}<Check size={14} class="copied" />{:else}<Copy size={14} />{/if}
                </Tooltip.Trigger>
                <Tooltip.Portal>
                  <Tooltip.Content class="tooltip" sideOffset={6} data-slop-export="hide">{copiedField === "email" ? "Copied" : "Copy email"}</Tooltip.Content>
                </Tooltip.Portal>
              </Tooltip.Root>
            </li>
            <li class="row">
              <div class="bubble"><Phone size={14} /></div>
              <div class="fields">
                <span class="label">Phone</span>
                <input class="valueInput" aria-label="Phone number" placeholder="(555) 000-0000" use:bindValue={doc.fields.phone} />
              </div>
              <Tooltip.Root disableCloseOnTriggerClick>
                <Tooltip.Trigger class="actionBtn" data-slop-export="hide" aria-label={copiedField === "phone" ? "Phone copied" : "Copy phone"} disabled={!doc.current.phone.trim()} onclick={() => copyToClipboard(doc.current.phone, "phone")}>
                  {#if copiedField === "phone"}<Check size={14} class="copied" />{:else}<Copy size={14} />{/if}
                </Tooltip.Trigger>
                <Tooltip.Portal>
                  <Tooltip.Content class="tooltip" sideOffset={6} data-slop-export="hide">{copiedField === "phone" ? "Copied" : "Copy phone"}</Tooltip.Content>
                </Tooltip.Portal>
              </Tooltip.Root>
            </li>
            <li class="row">
              <div class="bubble"><Globe size={14} /></div>
              <div class="fields">
                <span class="label">Website</span>
                <input class="valueInput" aria-label="Personal website" placeholder="yoursite.com" use:bindValue={doc.fields.website} />
              </div>
              <a class="actionBtn" data-slop-export="hide" aria-label="Open website" aria-disabled={!websiteHref} href={websiteHref || undefined} target="_blank" rel="noopener noreferrer">
                <ExternalLink size={14} />
              </a>
            </li>
          </ul>

          <footer class="footer">
            <div class="circles">
              <a class="circleBtn" href={mailHref || undefined} aria-label="Compose email to {doc.current.name || 'this contact'}" aria-disabled={!mailHref}><Mail size={16} /></a>
              <a class="circleBtn" href={phoneHref || undefined} aria-label="Call {doc.current.name || 'this contact'}" aria-disabled={!phoneHref}><Phone size={16} /></a>
              <a class="circleBtn" href={websiteHref || undefined} target="_blank" rel="noopener noreferrer" aria-label="Visit website" aria-disabled={!websiteHref}><Globe size={16} /></a>
            </div>
            <Button.Root type="button" class="vcardBtn" data-slop-export="hide" onclick={downloadVCard} aria-label="Save contact to Address Book">
              <Download size={14} />
              <span>Save vCard</span>
            </Button.Root>
          </footer>
        </article>
      </div>
    </main>
  </Tooltip.Provider>

  {#snippet exportView()}
    {@const data = doc.current}
    {@const hasChannel = Boolean(data.email.trim() || data.phone.trim() || data.website.trim())}
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
  {/snippet}

  {#snippet icon()}
    <div class="iconSurface" aria-hidden="true">
      <div class="iconPlate">
        <div class="iconBadge">
          <div class="iconNotch"></div>
          <span class="iconTab">{tabLetter}</span>
          <div class="iconAvatar">{tabLetter}</div>
          <div class="iconTextGroup">
            <span class="iconNameBar"></span>
            <span class="iconSubBar"></span>
          </div>
          <div class="iconLines">
            <span class="iconRow"></span>
            <span class="iconRow"></span>
            <span class="iconRow"></span>
          </div>
          <div class="iconButtons">
            <span class="iconCircle"></span>
            <span class="iconCircle"></span>
            <span class="iconCircle"></span>
          </div>
        </div>
      </div>
    </div>
  {/snippet}
</Slop>
