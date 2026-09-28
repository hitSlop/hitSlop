<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import { fly } from "svelte/transition";
  import { MERCH_CATEGORIES, merchAsset, type MerchPiece, type MerchCategory, type MerchAssetKind } from "../merch/pieces";

  let { pieces, notifyEndpoint }: { pieces: MerchPiece[]; notifyEndpoint?: string } = $props();
  const initial = untrack(() => pieces[0]);
  let selectedSlug = $state(initial.slug);
  let opened = $state(initial);
  let previewUrl = $state(merchAsset(initial.slug, initial.defaultView));
  let previewIsModel = $state(initial.defaultView === "model");
  let previewKind = $state<MerchAssetKind>(initial.defaultView);
  const previewAlt = $derived(opened.views?.find((view) => view.kind === previewKind)?.alt ?? (previewIsModel ? `Model wearing ${opened.name}` : opened.name));
  let previewError = $state(false);
  let loading = $state(false);
  let loadingError = $state("");
  let request = 0;
  let motionMs = $state(240);
  let compact = $state(true);
  let capacity = $state(3);
  let columns = $state(3);
  let category = $state<MerchCategory>(initial.category);
  let page = $state(0);
  let shelf: HTMLDivElement;
  let desktop: HTMLDivElement;
  let signup: HTMLDialogElement;
  const fileButtons: Record<string, HTMLButtonElement> = {};
  const groups = $derived(MERCH_CATEGORIES.map((group) => ({ ...group, items: pieces.filter((piece) => piece.category === group.id) })).filter((group) => group.items.length));
  const categoryItems = $derived(pieces.filter((piece) => piece.category === category));
  const pages = $derived(Math.max(1, Math.ceil(categoryItems.length / capacity)));
  const visibleGroups = $derived(compact ? groups.filter((group) => group.id === category).map((group) => ({ ...group, items: group.items.slice(page * capacity, (page + 1) * capacity) })) : groups);
  const visibleItems = $derived(visibleGroups.flatMap((group) => group.items));
  const selected = $derived(pieces.find((piece) => piece.slug === selectedSlug) ?? initial);

  // Two pieces are literally clickers. Go on.
  const clickers = new Set(["15-cursor-clicker", "16-ok-again"]);
  let clicks = $state(0);
  let pressed = $state(false);
  function click(): void {
    clicks += 1;
    pressed = true;
    setTimeout(() => { pressed = false; }, 110);
  }

  // Shuffle through the wardrobe like a fitting-room montage, then land on something.
  let spinning = $state(false);
  async function surprise(): Promise<void> {
    if (spinning) return;
    spinning = true;
    const pick = pieces[Math.floor(Math.random() * pieces.length)]!;
    const hops = motionMs ? 11 : 0;
    for (let i = 0; i < hops; i++) {
      const pool = visibleItems.length ? visibleItems : pieces;
      selectedSlug = pool[Math.floor(Math.random() * pool.length)]!.slug;
      await new Promise((resolve) => setTimeout(resolve, 55 + i * 16));
    }
    category = pick.category;
    await tick();
    page = Math.max(0, Math.floor(categoryItems.findIndex((entry) => entry.slug === pick.slug) / capacity));
    await openPiece(pick);
    fileButtons[pick.slug]?.focus({ preventScroll: true });
    spinning = false;
  }

  let email = $state("");
  let notifyState = $state<"idle" | "sending" | "done" | "error">("idle");
  let notifiedPiece = $state("");
  let signupController: AbortController | undefined;

  async function decodeImage(url: string): Promise<void> {
    const image = new Image();
    image.src = url;
    await image.decode();
  }

  async function openPiece(piece: MerchPiece, productOnly = false, updateHash = true, view?: MerchAssetKind): Promise<void> {
    selectedSlug = piece.slug;
    const currentRequest = ++request;
    loading = true;
    loadingError = "";
    let kind: MerchAssetKind = productOnly ? "thumb" : view ?? piece.defaultView;
    let url = merchAsset(piece.slug, kind);
    try {
      try {
        await decodeImage(url);
      } catch (error) {
        if (kind === "thumb") throw error;
        kind = "thumb";
        url = merchAsset(piece.slug, "thumb");
        await decodeImage(url);
      }
      if (currentRequest !== request) return;
      opened = piece;
      previewUrl = url;
      previewKind = kind;
      previewIsModel = kind.startsWith("model");
      previewError = false;
      if (updateHash) history.replaceState(null, "", `#${piece.slug}`);
    } catch {
      if (currentRequest === request) loadingError = `Couldn't open ${piece.name}. Try opening it again.`;
    } finally {
      if (currentRequest === request) loading = false;
    }
  }

  function pickerKey(event: KeyboardEvent): void {
    const index = visibleItems.findIndex((piece) => piece.slug === selectedSlug);
    if (!compact && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
      event.preventDefault();
      const groupIndex = visibleGroups.findIndex((group) => group.items.some((piece) => piece.slug === selectedSlug));
      if (groupIndex < 0) return;
      const itemIndex = visibleGroups[groupIndex].items.findIndex((piece) => piece.slug === selectedSlug);
      const direction = event.key === "ArrowDown" ? 1 : -1;
      const nextGroup = visibleGroups[(groupIndex + direction + visibleGroups.length) % visibleGroups.length];
      const next = nextGroup.items[Math.min(itemIndex, nextGroup.items.length - 1)];
      selectedSlug = next.slug;
      fileButtons[next.slug]?.focus();
      return;
    }
    const step = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: columns, ArrowUp: -columns }[event.key];
    if (!step && event.key !== "Home" && event.key !== "End") return;
    event.preventDefault();
    const nextIndex = event.key === "Home" ? 0 : event.key === "End" ? visibleItems.length - 1 : (Math.max(index, 0) + (step ?? 0) + visibleItems.length) % visibleItems.length;
    const next = visibleItems[nextIndex];
    selectedSlug = next.slug;
    fileButtons[next.slug]?.focus();
  }

  function chooseCategory(next: MerchCategory): void {
    category = next;
    page = 0;
    const first = pieces.find((piece) => piece.category === next);
    if (first) selectedSlug = first.slug;
  }

  function changePage(next: number): void {
    page = Math.max(0, Math.min(pages - 1, next));
    const first = categoryItems[page * capacity];
    if (first) selectedSlug = first.slug;
  }

  async function notify(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (!notifyEndpoint || notifyState === "sending") return;
    const piece = opened;
    notifyState = "sending";
    signupController = new AbortController();
    const timeout = setTimeout(() => signupController?.abort(), 15000);
    try {
      const response = await fetch(notifyEndpoint, {
        method: "POST", headers: { "content-type": "application/json" },
        body: JSON.stringify({ email, piece: piece.slug }), signal: signupController.signal,
      });
      if (!response.ok) throw new Error(String(response.status));
      notifiedPiece = piece.name;
      notifyState = "done";
    } catch {
      notifyState = "error";
    } finally {
      clearTimeout(timeout);
    }
  }

  onMount(() => {
    const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
    const updateMotion = () => { motionMs = reducedMotion.matches ? 0 : 240; };
    updateMotion();
    reducedMotion.addEventListener("change", updateMotion);
    const resize = () => {
      const width = desktop.clientWidth;
      const height = shelf.clientHeight;
      columns = width >= 1100 ? 5 : width >= 760 ? 4 : 3;
      const allRows = groups.reduce((rows, group) => rows + Math.ceil(group.items.length / columns), 0);
      const wasCompact = compact;
      const oldCapacity = capacity;
      compact = width < 1100 || height < allRows * 104 + groups.length * 24;
      capacity = columns;
      if (compact && (!wasCompact || oldCapacity !== capacity)) {
        category = selected.category;
        page = Math.floor(categoryItems.findIndex((piece) => piece.slug === selectedSlug) / capacity);
      } else {
        page = Math.min(page, Math.max(0, Math.ceil(categoryItems.length / capacity) - 1));
      }
    };
    const observer = new ResizeObserver(resize);
    observer.observe(shelf);
    observer.observe(desktop);
    resize();
    const fromHash = async () => {
      let slug = "";
      try { slug = decodeURIComponent(location.hash.slice(1)); } catch { return; }
      const piece = pieces.find((entry) => entry.slug === slug) ?? initial;
      category = piece.category;
      await tick();
      page = Math.floor(categoryItems.findIndex((entry) => entry.slug === piece.slug) / capacity);
      void openPiece(piece, false, false);
    };
    // Also verify the server-rendered image: it may have failed before hydration.
    void openPiece(initial, false, false);
    void fromHash();
    window.addEventListener("hashchange", fromHash);
    const joinButton = document.getElementById("join-drop");
    const showSignup = () => { if (!signup.open) signup.showModal(); };
    joinButton?.addEventListener("click", showSignup);
    return () => {
      request++;
      observer.disconnect();
      reducedMotion.removeEventListener("change", updateMotion);
      window.removeEventListener("hashchange", fromHash);
      joinButton?.removeEventListener("click", showSignup);
      signupController?.abort();
    };
  });
</script>

<div class="desktop" bind:this={desktop} style:--file-columns={columns}>
  <header class="intro">
    <p class="edition"><span aria-hidden="true">✳</span> A little out of office.</p>
    <h1>Try on the chaos<span>.</span></h1>
    <button type="button" class="surprise" onclick={surprise} disabled={spinning}><span aria-hidden="true">🎲</span> {spinning ? "Rummaging…" : "Surprise me"}</button>
  </header>

  <div class="wardrobe" bind:this={shelf} class:compact>
    {#if compact}
      <div class="categories" aria-label="Product categories">
        {#each groups as group}
          <button type="button" aria-pressed={category === group.id} onclick={() => chooseCategory(group.id)}>{group.label}<span>{group.items.length}</span></button>
        {/each}
      </div>
    {/if}
    <div class="groups" role="group" aria-label="Merch files" style:grid-template-rows={compact ? "minmax(0, 1fr)" : groups.map((group) => `${Math.ceil(group.items.length / columns)}fr`).join(" ")}>
      {#each visibleGroups as group (group.id)}
        <section class="group" aria-label={group.label}>
          {#if !compact}<h2>{group.label}<span>{group.items.length} pieces</span></h2>{/if}
          <div class="files">
            {#each group.items as piece (piece.slug)}
              <button bind:this={fileButtons[piece.slug]} type="button" class="file-icon" class:selected={piece.slug === selectedSlug} aria-pressed={piece.slug === selectedSlug} aria-label={`${piece.name}, ${piece.file}`} title={piece.file} onkeydown={pickerKey} onclick={() => openPiece(piece)}>
                <span class="product"><img src={merchAsset(piece.slug, "thumb")} alt="" width="480" height="480" draggable="false" /></span>
                <span class="filename">{piece.file}</span>
              </button>
            {/each}
          </div>
        </section>
      {/each}
    </div>
    {#if compact}
      <div class="pagination">
        <span>{categoryItems.length} pieces <span class="page-count">/ {page + 1} of {pages}</span></span>
        <div><button type="button" aria-label="Previous products" disabled={page === 0} onclick={() => changePage(page - 1)}>←</button><button type="button" aria-label="Next products" disabled={page >= pages - 1} onclick={() => changePage(page + 1)}>→</button></div>
      </div>
    {:else}
      <p class="desktop-note"><span aria-hidden="true">↖</span> Good software. Weird clothes.</p>
    {/if}
  </div>

  <section class="preview" aria-label="Product preview" aria-busy={loading}>
    <div class="window-bar" class:has-views={!!opened.views?.length}>
      <span class="window-lights" aria-hidden="true"><i></i><i></i><i></i></span>
      <span class="window-filename">{opened.file}</span>
      {#if opened.views?.length}
        <div class="view-switch" aria-label={`${opened.name} views`}>
          {#each opened.views as view, index}
            <button type="button" aria-pressed={previewKind === view.kind || (previewKind === "thumb" && index === 0)} onclick={() => openPiece(opened, false, true, view.kind)}>{view.label}</button>
          {/each}
        </div>
      {:else}<span class="window-label">Preview</span>{/if}
    </div>
    <div class="stage" class:product-stage={!previewIsModel}>
      {#key previewUrl}
        <div class="shot" in:fly={{ y: 8, duration: motionMs }} out:fly={{ y: -4, duration: motionMs }}>
          {#if previewError}
            <div class="image-error"><p>This preview couldn’t load.</p><button type="button" onclick={() => openPiece(opened, true)}>Try again</button></div>
          {:else}
            <img class:model={previewIsModel} class:flat={!previewIsModel} src={previewUrl} alt={previewAlt} width={previewIsModel ? 800 : 480} height={previewIsModel ? 1200 : 480} onerror={() => { if (previewKind !== "thumb") void openPiece(opened, true); else previewError = true; }} />
          {/if}
        </div>
      {/key}
      {#if loading}<span class="loading">Opening {selected.name}…</span>{/if}
      {#if loadingError}<p class="load-error" role="alert">{loadingError}</p>{/if}
      <span class="preview-stamp" aria-hidden="true">hitSlop<br /><span>OFF THE DESKTOP.</span></span>
      <span class="sticker coming" data-tone="sun" style="--tilt: -8deg" aria-hidden="true">coming soon ✨</span>
      {#if clickers.has(opened.slug)}
        <button type="button" class="clicker" class:pressed onclick={click} aria-label={`Click the ${opened.name}. ${clicks} clicks so far.`}>
          <span class="click-count">{clicks === 0 ? "go on, click it" : `${clicks} click${clicks === 1 ? "" : "s"}${clicks >= 20 ? " (you ok?)" : ""}`}</span>
        </button>
      {/if}
    </div>
    <div class="caption" aria-live="polite">
      <div><h2>{opened.name}</h2><p>{opened.blurb}</p></div>
      <span class="piece-status"><i aria-hidden="true"></i>Coming soon</span>
    </div>
  </section>
</div>

<dialog id="merch-signup" bind:this={signup} aria-labelledby="signup-title">
  <form method="dialog" class="dialog-close"><button aria-label="Close signup">×</button></form>
  <p class="dialog-eyebrow">THE FIRST DROP</p>
  {#if !notifyEndpoint}
    <h2 id="signup-title">Signups opening soon<span>.</span></h2>
    <p>Good software. Weird clothes. The first hitSlop collection is on its way.</p>
    <form method="dialog"><button class="signup-button">Back to the wardrobe →</button></form>
  {:else if notifyState === "done"}
    <h2 id="signup-title">You’re on the list<span>.</span></h2>
    <p role="status">We’ll tell you when {notifiedPiece} drops.</p>
  {:else}
    <h2 id="signup-title">Get the first look<span>.</span></h2>
    <p>Leave your email. We’ll let you know when the merch drops.</p>
    <form onsubmit={notify}>
      <label for="notify-email">Your email</label>
      <input id="notify-email" name="email" type="email" required autocomplete="email" placeholder="you@example.com" bind:value={email} />
      <button class="signup-button" disabled={notifyState === "sending"}>{notifyState === "sending" ? "Joining…" : "Notify me →"}</button>
      {#if notifyState === "error"}<p role="alert">That didn’t go through. Please try again.</p>{/if}
    </form>
  {/if}
</dialog>

<style>
  .desktop { height: 100%; width: min(1800px, 100%); margin-inline: auto; padding: clamp(18px, 2.5vh, 32px) clamp(24px, 3vw, 48px); display: grid; grid-template-columns: minmax(0, 1.12fr) minmax(0, 1fr); grid-template-rows: auto minmax(0, 1fr); gap: 16px clamp(24px, 3vw, 48px); }
  .intro { min-width: 0; align-self: start; }
  .edition { margin: 0 0 8px; display: flex; align-items: center; gap: 9px; font-size: .75rem; font-weight: 700; letter-spacing: .08em; text-transform: uppercase; color: #5b358e; }
  .edition span { font-size: 24px; line-height: 1; }
  h1 { font-family: "HitSlop Display", sans-serif; font-size: clamp(38px, 4.5vw, 80px); font-weight: 800; line-height: .98; letter-spacing: -.025em; white-space: nowrap; }
  h1 > span { color: #7922e0; }
  .wardrobe { min-height: 0; min-width: 0; display: flex; flex-direction: column; gap: 8px; }
  .groups { min-height: 0; flex: 0 1 480px; display: grid; gap: 8px; }
  .group { min-height: 0; display: grid; grid-template-rows: auto minmax(0, 1fr); gap: 4px; }
  .group h2 { display: flex; align-items: center; justify-content: space-between; margin: 0; padding: 0 5px 8px; border-bottom: 1px solid #77749c30; font-size: .7rem; font-weight: 750; letter-spacing: .14em; text-transform: uppercase; }
  .group h2 span { font-weight: 500; font-size: .64rem; letter-spacing: .035em; text-transform: none; color: #626480; }
  .files { min-height: 0; display: grid; grid-template-columns: repeat(var(--file-columns), minmax(0, 1fr)); grid-auto-rows: minmax(0, 1fr); gap: 4px; }
  .file-icon { min-width: 0; min-height: 0; width: 100%; padding: 4px; border: 1px solid transparent; border-radius: 7px; background: transparent; color: inherit; cursor: pointer; display: grid; grid-template-rows: minmax(0, 1fr) auto; gap: 2px; transition: background 150ms, border-color 150ms; }
  .product { min-height: 0; min-width: 0; display: flex; justify-content: center; align-items: center; }
  .product img { height: 100%; width: 100%; max-width: 144px; max-height: 128px; object-fit: contain; filter: drop-shadow(0 4px 5px #29203f18); pointer-events: none; transition: transform 320ms cubic-bezier(.34, 1.56, .64, 1); }
  .filename { justify-self: center; max-width: 100%; padding: 2px 5px; border-radius: 3px; overflow-wrap: anywhere; font-size: clamp(11px, .85vw, 13px); line-height: 1.3; text-align: center; }
  .file-icon:hover { background: #ffffff28; }
  .file-icon:hover img { transform: translateY(-5px) rotate(-5deg) scale(1.06); }
  .file-icon:active img { transform: translateY(1px) scale(.96); }
  .intro { display: flex; flex-wrap: wrap; align-items: end; justify-content: space-between; gap: 10px 18px; }
  .intro .edition { flex-basis: 100%; }
  .surprise { min-height: 44px; padding: 0 16px; display: inline-flex; align-items: center; gap: 8px; border: 2px solid #15152e; border-radius: 16px; color: #15152e; background: #fff; box-shadow: 0 3px 0 #15152e; font-family: "HitSlop Display", sans-serif; font-size: 1rem; font-weight: 800; cursor: pointer; transition: transform 220ms cubic-bezier(.34, 1.56, .64, 1); }
  .surprise:hover { transform: translateY(-2px) rotate(2deg); }
  .surprise:disabled { cursor: progress; }
  .stage .coming { position: absolute; top: 16px; left: 16px; z-index: 2; font-size: .95rem; pointer-events: none; }
  .clicker { position: absolute; inset: 0; z-index: 1; display: flex; align-items: flex-end; justify-content: center; padding: 0 0 18px; border: 0; background: transparent; cursor: pointer; }
  .click-count { padding: 6px 14px; border: 2px solid #15152e; border-radius: 99px; background: #fff; box-shadow: 0 3px 0 #15152e; font-family: "HitSlop Handwriting", cursive; font-size: 1.1rem; }
  .stage:has(.clicker.pressed) .shot img { transform: scale(.94) translateY(4px); transition: transform 90ms ease-out; }
  .shot img { transition: transform 200ms cubic-bezier(.34, 1.56, .64, 1); }
  .file-icon.selected { border-color: #8162c276; background: #ffffff35; }
  .file-icon.selected .filename { color: #fcf9ff; background: #7541c2; }
  button:focus-visible { outline: 3px solid #7138db; outline-offset: 2px; }
  .desktop-note { margin: 0; color: #644384; font-family: "HitSlop Handwriting", cursive; font-size: clamp(16px, 1.5vw, 23px); transform: rotate(-2deg); padding-left: 12px; }
  .desktop-note span { margin-right: 6px; }
  .categories { display: flex; gap: 4px; border-bottom: 1px solid #77749c30; }
  .categories button { min-height: 40px; padding: 0 12px; background: transparent; border: 0; border-bottom: 2px solid transparent; color: #56536f; font-size: .83rem; font-weight: 650; cursor: pointer; }
  .categories button[aria-pressed="true"] { border-bottom-color: #7541c2; color: #4e1c9c; }
  .categories button span { margin-left: 7px; font-size: .66rem; opacity: .7; }
  .compact .groups { flex: 1; }
  .compact .group { grid-template-rows: minmax(0, 1fr); }
  .compact .files { grid-auto-rows: minmax(0, 1fr); }
  .compact .product img { max-height: 220px; }
  .pagination { height: 32px; flex: 0 0 32px; display: flex; align-items: center; justify-content: space-between; color: #56536f; font-size: .75rem; }
  .page-count { color: #64617b; margin-left: 6px; }
  .pagination > div { display: flex; gap: 6px; }
  .pagination button { width: 36px; height: 30px; border: 1px solid #74698e3b; border-radius: 5px; background: #f9f8ff66; color: #33274b; font-size: 19px; cursor: pointer; }
  .pagination button:disabled { opacity: .3; cursor: default; }
  .preview { grid-column: 2; grid-row: 1 / 3; justify-self: center; width: 100%; max-width: 650px; min-width: 0; min-height: 0; display: grid; grid-template-rows: 38px minmax(0, 1fr) auto; align-self: stretch; overflow: hidden; border: 1px solid #ffffffed; border-radius: 12px; background: #f6f5fb; box-shadow: 0 2px 5px #35436818, 0 28px 70px -18px #35436855, 0 0 0 1px #595d8820; }
  .window-bar { position: relative; display: grid; grid-template-columns: 65px minmax(0, 1fr) 65px; align-items: center; gap: 8px; padding: 0 13px; border-bottom: 1px solid #8f90a22b; background: linear-gradient(#fcfbff, #eeedf6); font-size: .74rem; }
  .window-lights { display: flex; gap: 6px; }
  .window-lights i { width: 10px; height: 10px; border-radius: 50%; background: #ff6057; box-shadow: inset 0 0 0 1px #0000000d; }
  .window-lights i:nth-child(2) { background: #ffbd2e; }
  .window-lights i:nth-child(3) { background: #28c840; }
  .window-filename { text-align: center; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .window-label { text-align: right; color: #747082; font-size: .66rem; }
  .window-bar.has-views { grid-template-columns: 50px minmax(0, 1fr) auto; }
  .view-switch { display: flex; gap: 2px; }
  .view-switch button { min-height: 28px; padding: 2px 8px; border: 1px solid transparent; border-radius: 4px; color: #655975; background: transparent; font-size: .67rem; cursor: pointer; }
  .view-switch button[aria-pressed="true"] { color: #542a8d; border-color: #d1c2e6; background: #fdfaff; box-shadow: 0 1px 2px #3826490a; }
  .stage { position: relative; min-height: 0; overflow: hidden; background: linear-gradient(#9caae1, #a8b6ea 25%, #a3b1ea 50%, #a1afe7 75%, #c6d0f6 90%, #d8defc); }
  .stage.product-stage { background: #f4f2ee; }
  .shot { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; }
  .shot img { width: 100%; height: 100%; object-fit: contain; }
  .shot img.flat { width: 78%; height: 82%; filter: drop-shadow(0 8px 10px #29203f1a); }
  .preview-stamp { position: absolute; bottom: 14px; right: 16px; font-size: 22px; font-weight: 750; line-height: .75; letter-spacing: -.05em; color: #24234c75; text-align: right; pointer-events: none; }
  .preview-stamp span { font-size: 7px; letter-spacing: .12em; font-weight: 650; }
  .loading, .load-error { position: absolute; bottom: 12px; left: 12px; right: 12px; width: fit-content; max-width: calc(100% - 24px); padding: 6px 10px; border-radius: 5px; background: #f9f8fff2; color: #3c3156; font-size: .75rem; z-index: 2; }
  .load-error { margin: 0; }
  .caption { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 18px 22px; border-top: 1px solid #8f90a22b; }
  .caption h2 { font-size: 1.25rem; font-weight: 700; line-height: 1.2; letter-spacing: -.025em; }
  .caption p { margin: 5px 0 0; max-width: 34ch; color: #676174; font-size: .8rem; line-height: 1.4; }
  .piece-status { display: flex; align-items: center; gap: 5px; font-size: .65rem; white-space: nowrap; color: #656073; }
  .piece-status i { width: 6px; height: 6px; border-radius: 50%; background: #3eaf68; }
  .image-error { text-align: center; color: #2f3152; }
  .image-error button { padding: 8px 14px; border: 1px solid #777; border-radius: 6px; cursor: pointer; }
  dialog { width: min(440px, calc(100% - 32px)); max-height: calc(100dvh - 32px); padding: 36px; border: 1px solid #fff; border-radius: 14px; color: #1b1933; background: #f8f6ff; box-shadow: 0 30px 90px #23204444; }
  dialog::backdrop { background: #26214055; backdrop-filter: blur(5px); }
  .dialog-close { position: absolute; top: 8px; right: 10px; }
  .dialog-close button { width: 36px; height: 36px; border: 0; border-radius: 6px; background: transparent; font-size: 26px; color: #5d5372; cursor: pointer; }
  .dialog-eyebrow { margin: 0 0 16px; font-size: .7rem; font-weight: 700; letter-spacing: .12em; color: #7741b7; }
  dialog h2 { font-family: "HitSlop Display", sans-serif; font-size: 40px; font-weight: 800; line-height: 1; }
  dialog h2 span { color: #7922e0; }
  dialog p { font-size: .95rem; color: #5d5372; }
  dialog label { font-size: .8rem; font-weight: 650; }
  dialog input { display: block; width: 100%; margin-top: 6px; padding: 13px; border: 1px solid #cec7df; border-radius: 7px; background: #fff; color: #28243a; font-size: 16px; }
  .signup-button { margin-top: 18px; padding: 12px 16px; min-height: 44px; border: 1px solid #e6ca45; border-radius: 7px; background: #ffe66b; color: #28243a; font-size: .9rem; font-weight: 700; cursor: pointer; }
  .signup-button:disabled { opacity: .6; cursor: wait; }
  @media (max-width: 1099px) { .desktop { column-gap: 32px; padding: 24px; } h1 { font-size: clamp(32px, 4.4vw, 48px); } .edition { font-size: .65rem; } .caption { padding: 14px; gap: 10px; } .piece-status { display: none; } }
  @media (max-width: 759px) {
    .desktop { padding: 14px 18px 10px; grid-template-columns: minmax(0, 1fr); grid-template-rows: auto minmax(0, 1fr) 190px; gap: 14px; }
    .intro { grid-row: 1; }
    .edition { display: none; }
    h1 { font-size: clamp(30px, 7.9vw, 46px); }
    .preview { grid-column: 1; grid-row: 2; max-width: 580px; grid-template-rows: 30px minmax(0, 1fr) auto; border-radius: 9px; box-shadow: 0 10px 24px -12px #35436855; }
    .window-bar { padding-inline: 10px; font-size: .65rem; }
    .window-lights i { width: 8px; height: 8px; }
    .window-label { font-size: .6rem; }
    .caption { padding: 9px 12px; }
    .caption h2 { font-size: .95rem; }
    .caption p { max-width: none; font-size: .69rem; margin-top: 3px; }
    .preview-stamp { bottom: 10px; right: 10px; font-size: 16px; }
    .preview-stamp span { font-size: 5px; }
    .wardrobe { grid-row: 3; gap: 5px; }
    .categories { justify-content: space-between; }
    .categories button { min-height: 34px; padding: 0 8px; font-size: .77rem; }
    .groups { gap: 0; }
    .files { gap: 3px; }
    .file-icon { padding: 3px; gap: 2px; }
    .filename { font-size: 10px; padding: 1px 3px; }
    .pagination { height: 28px; flex-basis: 28px; font-size: .67rem; }
    .pagination button { height: 28px; }
  }
  @media (max-height: 700px) and (min-width: 760px) { .desktop { padding-block: 16px; gap: 14px 32px; } .edition { display: none; } h1 { font-size: 42px; } .wardrobe { gap: 8px; } }
  @media (max-height: 700px) and (max-width: 759px) { .desktop { gap: 10px; padding-top: 10px; grid-template-rows: auto minmax(0, 1fr) 174px; } .preview-stamp { display: none; } }
  @media (hover: none) { .file-icon:hover img { transform: none; } }
  @media (prefers-reduced-motion: reduce) { .file-icon, .product img { transition: none; } }
</style>
