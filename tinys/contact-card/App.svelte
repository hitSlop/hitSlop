<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
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
  import { attachments, bindText, getDocument } from "@hitslop/tinys/svelte";
  import schema from "./schema";
  import { cardView, setPortrait, vCardFileName, vCardText } from "./shared";

  const doc = getDocument(schema);
  let copiedField = $state<string | null>(null);
  let fileInput = $state<HTMLInputElement>();

  const card = $derived(cardView(doc.current));
  // The portrait is a same-origin attachment URL; the export mounts its own image of it.
  const portraitSrc = $derived(card.portraitId ? attachments.url(card.portraitId) : "");

  $effect(() => {
    if (!copiedField) return;
    const field = copiedField;
    const timer = window.setTimeout(() => { if (copiedField === field) copiedField = null; }, 1800);
    return () => window.clearTimeout(timer);
  });

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
    const blob = new Blob([vCardText(doc.current)], { type: "text/vcard;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = vCardFileName(doc.current);
    link.click();
    URL.revokeObjectURL(url);
  }
  function onPortrait(event: Event) {
    const input = event.currentTarget;
    if (!(input instanceof HTMLInputElement)) return;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    void attachments.import<typeof schema.descriptor>(file, (tx, ref) => setPortrait(tx, ref));
  }
</script>

{#snippet portrait(editable: boolean)}
  {#if portraitSrc}
    <img class="avatarImg" src={portraitSrc} alt="" />
  {:else if card.monogram}
    <span class="monogram">{card.monogram}</span>
  {:else}
    <svg class="avatarImg" viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <rect width="100" height="100" fill="var(--tiny-avatarFill)" />
      <circle cx="50" cy="38" r="18" fill="var(--tiny-avatarInk)" />
      <path d="M22 84C22 68 34 58 50 58C66 58 78 68 78 84" fill="var(--tiny-avatarInk)" />
    </svg>
  {/if}
  {#if editable}
    <span class="overlay" data-tiny-export="hide"><Camera size={20} /></span>
  {/if}
{/snippet}

  <Tooltip.Provider delayDuration={200}>
    <main class="canvas" aria-label="Contact card">
      <input hidden type="file" accept="image/*" bind:this={fileInput} data-tiny-export="hide" onchange={onPortrait} />
      <div class="tray">
        <span class="indexTab" aria-hidden="true">{card.tabLetter}</span>
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
                <input class="valueInput" aria-label="Email address" placeholder="name@example.com" bind:value={doc.fields.email.value} />
              </div>
              <Tooltip.Root disableCloseOnTriggerClick>
                <Tooltip.Trigger class="actionBtn" data-tiny-export="hide" aria-label={copiedField === "email" ? "Email copied" : "Copy email"} disabled={!doc.current.email.trim()} onclick={() => copyToClipboard(doc.current.email, "email")}>
                  {#if copiedField === "email"}<Check size={14} class="copied" />{:else}<Copy size={14} />{/if}
                </Tooltip.Trigger>
                <Tooltip.Portal>
                  <Tooltip.Content class="tooltip" sideOffset={6} data-tiny-export="hide">{copiedField === "email" ? "Copied" : "Copy email"}</Tooltip.Content>
                </Tooltip.Portal>
              </Tooltip.Root>
            </li>
            <li class="row">
              <div class="bubble"><Phone size={14} /></div>
              <div class="fields">
                <span class="label">Phone</span>
                <input class="valueInput" aria-label="Phone number" placeholder="(555) 000-0000" bind:value={doc.fields.phone.value} />
              </div>
              <Tooltip.Root disableCloseOnTriggerClick>
                <Tooltip.Trigger class="actionBtn" data-tiny-export="hide" aria-label={copiedField === "phone" ? "Phone copied" : "Copy phone"} disabled={!doc.current.phone.trim()} onclick={() => copyToClipboard(doc.current.phone, "phone")}>
                  {#if copiedField === "phone"}<Check size={14} class="copied" />{:else}<Copy size={14} />{/if}
                </Tooltip.Trigger>
                <Tooltip.Portal>
                  <Tooltip.Content class="tooltip" sideOffset={6} data-tiny-export="hide">{copiedField === "phone" ? "Copied" : "Copy phone"}</Tooltip.Content>
                </Tooltip.Portal>
              </Tooltip.Root>
            </li>
            <li class="row">
              <div class="bubble"><Globe size={14} /></div>
              <div class="fields">
                <span class="label">Website</span>
                <input class="valueInput" aria-label="Personal website" placeholder="yoursite.com" bind:value={doc.fields.website.value} />
              </div>
              <a class="actionBtn" data-tiny-export="hide" aria-label="Open website" aria-disabled={!card.websiteHref} href={card.websiteHref || undefined} target="_blank" rel="noopener noreferrer">
                <ExternalLink size={14} />
              </a>
            </li>
          </ul>

          <footer class="footer">
            <div class="circles">
              <a class="circleBtn" href={card.mailHref || undefined} aria-label="Compose email to {doc.current.name || 'this contact'}" aria-disabled={!card.mailHref}><Mail size={16} /></a>
              <a class="circleBtn" href={card.phoneHref || undefined} aria-label="Call {doc.current.name || 'this contact'}" aria-disabled={!card.phoneHref}><Phone size={16} /></a>
              <a class="circleBtn" href={card.websiteHref || undefined} target="_blank" rel="noopener noreferrer" aria-label="Visit website" aria-disabled={!card.websiteHref}><Globe size={16} /></a>
            </div>
            <Button.Root type="button" class="vcardBtn" data-tiny-export="hide" onclick={downloadVCard} aria-label="Save contact to Address Book">
              <Download size={14} />
              <span>Save vCard</span>
            </Button.Root>
          </footer>
        </article>
      </div>
    </main>
  </Tooltip.Provider>
