<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { bindText, getDocument } from "hitslop/svelte";
  import { Select, Slider, Button } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import Check from "@lucide/svelte/icons/check";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import schema, { kinds } from "./schema";
  import { addProduct, expiryText, isExpired, isKind, kindLabel, monthsOf, removeProduct, setLeft, setOpened, setPao, setShade, stashView, swatches, type Product } from "./pan";
  import Pan from "./Pan.svelte";

  const doc = getDocument(schema);

  // The pan being looked at is view state: it is not saved, so an export shows every product.
  let selectedId = $state<string | undefined>();
  const nowMonths = monthsOf(new Date());

  const kindItems = kinds.map((value) => ({ value, label: kindLabel[value] }));
  const { active, empties, next } = $derived(stashView(doc.current));
  const selected = $derived(doc.current.products.find((product) => product.$id === selectedId));

  async function add() {
    const { id } = await doc.change((tx) => addProduct(tx, {}, doc.current.products.length));
    selectedId = id;
  }

  async function remove(product: Product) {
    selectedId = undefined;
    await doc.change((tx) => removeProduct(tx, product));
  }
</script>

<main class="desk" aria-label="Project pan">
  <section class="compact">
    <div class="hinge" aria-hidden="true"></div>

    <header class="mirror">
      <input class="title" aria-label="Project title" placeholder="Project Pan" use:bindText={doc.fields.title} />
      <p class="count"><b>{empties.length}</b> panned · <b>{active.length}</b> in use</p>
      {#if next}<p class="next">Next to finish: <b>{next.name || "Unnamed"}</b>, {next.left}% left</p>{/if}
    </header>

    <div class="tray" aria-label="Products in use">
      {#each active as product (product.$id)}
        <Pan {product} selected={selectedId === product.$id} onselect={() => (selectedId = product.$id)} />
      {/each}
      <button class="pan add-pan" aria-label="Add a product" onclick={add}>
        <span class="dish add"><Plus size={22} /></span>
        <span class="pan-name">Add product</span>
      </button>
    </div>

    {#if selected}
      {@const row = doc.at(selected)}
      <section class="detail" aria-label={`Edit ${selected.name || "product"}`}>
        <div class="line">
          <input class="name" aria-label="Product name" placeholder="Product name" use:bindText={row.name} />
          <Button.Root class="trash" aria-label={`Remove ${selected.name || "product"}`} onclick={() => remove(selected)}><Trash2 size={16} /></Button.Root>
        </div>
        <div class="line">
          <input class="brand" aria-label="Brand" placeholder="Brand" use:bindText={row.brand} />
          <Select.Root type="single" value={selected.kind} items={kindItems} onValueChange={(value) => { if (isKind(value)) row.kind.set(value); }}>
            <Select.Trigger class="kind" aria-label="Kind of product">{kindLabel[selected.kind]}<ChevronDown size={14} /></Select.Trigger>
            <Select.Portal>
              <Select.Content class="menu" sideOffset={6}>
                <Select.Viewport>
                  {#each kindItems as item (item.value)}
                    <Select.Item value={item.value} label={item.label} class="menu-item">
                      {#snippet children({ selected: on })}<span>{item.label}</span>{#if on}<Check size={14} />{/if}{/snippet}
                    </Select.Item>
                  {/each}
                </Select.Viewport>
              </Select.Content>
            </Select.Portal>
          </Select.Root>
        </div>

        <div class="level">
          <span class="readout" aria-live="polite"><b>{selected.left}</b>% left</span>
          <Slider.Root type="single" min={0} max={100} step={1} value={selected.left} onValueChange={(value) => { row.left.value = value; }} class="slider" aria-label="Percent left">
            <span data-slider-track><Slider.Range /></span>
            <Slider.Thumb index={0} aria-label="Percent left" />
          </Slider.Root>
          <div class="quick">
            <Button.Root class="mini" disabled={selected.left === 0} onclick={() => doc.change((tx) => setLeft(tx, selected, Math.max(0, selected.left - 10)))}>−10%</Button.Root>
            <Button.Root class="mini hit" disabled={selected.left === 0} onclick={() => doc.change((tx) => setLeft(tx, selected, 0))}>Hit pan</Button.Root>
          </div>
        </div>

        <div class="line dates">
          <label>Opened<input type="date" value={selected.opened ?? ""} onchange={(event) => doc.change((tx) => setOpened(tx, selected, event.currentTarget.value))} /></label>
          <label>Good for
            <span class="stepper">
              <button aria-label="Fewer months" onclick={() => doc.change((tx) => setPao(tx, selected, selected.pao - 1))} disabled={selected.pao <= 1}>−</button>
              <b>{selected.pao}M</b>
              <button aria-label="More months" onclick={() => doc.change((tx) => setPao(tx, selected, selected.pao + 1))} disabled={selected.pao >= 60}>+</button>
            </span>
          </label>
        </div>
        <p class="expiry" data-warn={isExpired(selected, nowMonths) ? "" : undefined}>{expiryText(selected, nowMonths)}</p>

        <div class="swatches" role="radiogroup" aria-label="Shade">
          {#each swatches as shade}
            <button class="swatch" role="radio" aria-checked={selected.shade === shade} aria-label={`Shade ${shade}`} style:--shade={shade} onclick={() => doc.change((tx) => setShade(tx, selected, shade))}></button>
          {/each}
        </div>
      </section>
    {:else if !doc.current.products.length}
      <p class="hint">Add your first product to start your pan.</p>
    {:else}
      <p class="hint">Tap a pan to edit it.</p>
    {/if}

    <section class="shelf" aria-label="Empties">
      <h2>Empties <span>{empties.length}</span></h2>
      <div class="shelf-row">
        {#each empties as product (product.$id)}
          <Pan {product} selected={selectedId === product.$id} onselect={() => (selectedId = product.$id)} />
        {:else}
          <p class="none">Finished products land here.</p>
        {/each}
      </div>
    </section>
  </section>
</main>
