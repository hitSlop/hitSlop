<script lang="ts">
  import type { CaptureProps, WidgetProps } from "hitslop/svelte";
  import Check from "@lucide/svelte/icons/check";
  import Sparkle from "@lucide/svelte/icons/sparkle";
  import Brand from "./Brand.svelte";
  import Progress from "./Progress.svelte";
  import type schema from "./schema";
  import { checklistView } from "./tasks";
  let { snapshot, family }: CaptureProps<typeof schema> & WidgetProps = $props();
  const { visible, finished, ratio } = $derived(checklistView(snapshot));
  const rows = $derived(visible.slice(0, family === "small" ? 1 : family === "medium" ? 2 : 6));
</script>

<main class="widget" data-family={family}>
  {#if family === "large"}<Brand />{/if}
  <section class="paper">
    <h1>{snapshot.title || "Untitled list"}</h1>
    {#if family === "large"}
      <Progress total={visible.length} {finished} fill={ratio} />
    {:else}
      <p class="count">{visible.length - finished} left to do <span><Sparkle size={12} fill="currentColor" strokeWidth={0} /></span></p>
    {/if}
    <ol>
      {#each rows as task (task.$id)}
        <li data-done={task.done}>
          <span class="box" aria-label={task.done ? "Complete" : "Incomplete"}>{#if task.done}<Check size={12} strokeWidth={3} />{/if}</span>
          <span class="task">{task.text || "Untitled task"}</span>
        </li>
      {/each}
    </ol>
    {#if !visible.length}<p class="empty">A little breathing room.</p>{/if}
    {#if family === "large" && visible.length > rows.length}<p class="more">+ {visible.length - rows.length} more</p>{/if}
  </section>
</main>

<style>
  .widget { box-sizing: border-box; width: 100%; height: 100%; padding: 14px; display: flex; flex-direction: column; gap: 12px; color: var(--slop-ink); background: linear-gradient(145deg, color-mix(in srgb, var(--slop-paper) 10%, transparent), transparent 65%), var(--slop-surface); font-family: var(--font); }
  .widget :global(.checklist-header) { margin: 0; }
  .paper { min-height: 0; flex: 1; overflow: hidden; padding: 14px; border: 1px solid var(--slop-controlBorder); border-radius: 9px; background: var(--slop-paper); box-shadow: 0 2px 0 color-mix(in srgb, var(--slop-ink) 9%, transparent); }
  h1 { margin: 0 0 8px; font-family: var(--headingFont); font-weight: 700; font-size: 24px; line-height: 1.08; letter-spacing: -0.025em; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; overflow: hidden; overflow-wrap: anywhere; }
  .count { display: flex; justify-content: space-between; margin: 0 0 8px; font-size: 12px; font-weight: 600; color: var(--slop-muted); }
  .count span { color: var(--slop-accent); }
  ol { list-style: none; margin: 8px 0 0; padding: 0; }
  li { display: flex; gap: 8px; align-items: center; padding: 7px 0; border-top: 1px solid var(--slop-rule); font-size: 13px; line-height: 1.25; }
  .box { display: flex; align-items: center; justify-content: center; box-sizing: border-box; flex-shrink: 0; width: 16px; height: 16px; border: 1px solid var(--slop-successBorder); border-radius: 4px; text-align: center; font-weight: 800; }
  [data-done="true"] .box { background: var(--slop-success); }
  .task { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  [data-done="true"] .task { text-decoration: line-through; color: var(--slop-muted); }
  .empty, .more { font-size: 12px; color: var(--slop-muted); }
  [data-family="medium"] { padding: 10px; }
  [data-family="medium"] .paper { padding: 10px 14px; }
  [data-family="medium"] h1 { font-size: 20px; -webkit-line-clamp: 1; line-clamp: 1; margin-bottom: 4px; }
  [data-family="medium"] li { padding: 5px 0; }
  [data-family="small"] { padding: 8px; }
  [data-family="small"] .paper { padding: 10px; }
  [data-family="small"] h1 { font-size: 19px; }
  [data-family="small"] .task { white-space: normal; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; }
</style>
