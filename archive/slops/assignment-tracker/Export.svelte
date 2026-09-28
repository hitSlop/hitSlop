<script lang="ts">
  import { useDocument } from "@hitslop/document/svelte";
  import schema from "./schema";
  import { groupAssignments, todayISO, relativeDue } from "./due";

  let { reference = todayISO() }: { reference?: string } = $props();
  const doc = useDocument(schema);
  const data = $derived(doc.current);
  const stacks = $derived(groupAssignments(data.assignments, reference));
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
    {#each stacks as stack}<section class={"asg-stack"}>
        <div class={"asg-stackHead"}>
          <h2 class={"asg-stackLabel"} data-urgency={stack.id}>{stack.label}</h2>
          <span class={"asg-stackCount"}>{stack.items.length}</span>
        </div>
        {#each stack.items as item}<div class={"asg-exportRow"}>
            <span
              class={"asg-checkbox"}
              data-state={item.completed ? "checked" : "unchecked"}
              >{item.completed ? "✓" : ""}</span
            >
            <div class={"asg-exportBody"}>
              <strong class={"asg-itemTitle"}
                >{item.title || "Untitled assignment"}</strong
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
              {#if item.notes}<p class={"asg-exportNotes"}>{item.notes}</p>{/if}
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
