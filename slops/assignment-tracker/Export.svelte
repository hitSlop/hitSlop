<script lang="ts">
  import { previewText } from "../_shared/preview";
  let { preview = false }: { preview?: boolean } = $props();
  import doc from "./schema";
  import { todayISO, relativeDue, trackerView } from "./due";

  const reference = todayISO();
  const data = $derived(doc.current);
  // The export shows every stack, completed included; the window's open tab is not visible here.
  const stacks = $derived(trackerView(data, reference).all);
</script>

<article class={"asg-exportPad"} aria-label="Exported assignment tracker">
  <header class={"asg-header"}>
    <div class={"asg-identity"}>
      <span class={"asg-eyebrow"}>ASSIGNMENTS / STUDY PAD</span>
      <h1
        class={"asg-student"}
        style="white-space:normal;overflow-wrap:anywhere;margin:0"
      >
        {data.studentName || "Your study pad"}
      </h1>
      <span class={"asg-term"}>{data.term}</span>
    </div>
  </header>
  <div class={"asg-exportStacks"}>
    {#each (preview ? (stacks).slice(0, 3) : stacks) as stack}<section class={"asg-stack"}>
        <div class={"asg-stackHead"}>
          <h2 class={"asg-stackLabel"} data-urgency={stack.id}>{stack.label}</h2>
          <span class={"asg-stackCount"}>{stack.items.length}</span>
        </div>
        {#each (preview ? (stack.items).slice(0, 3) : stack.items) as item}<div class={"asg-exportRow"}>
            <span
              class={"asg-checkbox"}
              data-state={item.completed ? "checked" : "unchecked"}
              >{item.completed ? "✓" : ""}</span
            >
            <div class={"asg-exportBody"}>
              <strong class={"asg-itemTitle"}
                >{previewText(item.title || "Untitled assignment", preview, 100)}</strong
              >
              <div class={"asg-itemMeta"}>
                <span class={"asg-courseChip"}
                  >{item.courseCode || "No course"}</span
                ><span>{item.category}</span><span>{item.points} pts</span><span
                  >{item.dueDate || "No due date"} · {item.completed
                    ? "Completed"
                    : relativeDue(item.dueDate, reference).label}</span
                >
              </div>
              {#if item.notes}<p class={"asg-exportNotes"}>{previewText(item.notes, preview, 100)}</p>{/if}
            </div>
          </div>{/each}
      </section>{:else}<div class={"asg-empty"}>
        <h2>No assignments yet.</h2>
      </div>{/each}
  </div>
  <footer class={"asg-footer"}>
    <span>One thing at a time.</span><span
      >{data.assignments.filter((i) => i.completed).length} / {data.assignments
        .length} complete</span
    >
  </footer>
</article>
