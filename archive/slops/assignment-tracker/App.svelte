<script lang="ts">
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { Tabs, Checkbox, Dialog, Select } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import Check from "@lucide/svelte/icons/check";
  import ArrowUpRight from "@lucide/svelte/icons/arrow-up-right";
  import schema, { type Assignment } from "./schema";
  import { todayISO, relativeDue, parseISO, groupAssignments, msUntilNextDay } from "./due";
  import DuePicker from "./DuePicker.svelte";
  import Icon from "./Icon.svelte";
  import Export from "./Export.svelte";

  const doc = useDocument(schema);
  let filter = $state("todo");
  let day = $state(todayISO());
  $effect(() => {
    let timer: ReturnType<typeof setTimeout>;
    const refresh = () => {
      clearTimeout(timer);
      day = todayISO();
      timer = setTimeout(refresh, msUntilNextDay(new Date()));
    };
    const visible = () => {
      if (document.visibilityState === "visible") refresh();
    };
    refresh();
    document.addEventListener("visibilitychange", visible);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("visibilitychange", visible);
    };
  });
  const active = $derived(doc.current.assignments.filter((item) => !item.completed));
  const completed = $derived(doc.current.assignments.filter((item) => item.completed));
  const dueToday = $derived(active.filter((item) => relativeDue(item.dueDate, day).urgency === "today").length);
  const overdue = $derived(active.filter((item) => relativeDue(item.dueDate, day).urgency === "overdue").length);
  const readout = $derived(
    overdue ? `${overdue} overdue` : dueToday ? `${dueToday} due today` : active.length ? `${active.length} to do` : "All caught up",
  );
  const stacks = $derived(groupAssignments(filter === "done" ? completed : active, day));
  let open = $state(false);
  let fresh = $state(false);
  let error = $state("");
  let draft = $state({
    $id: "",
    courseCode: "",
    title: "",
    dueDate: "",
    category: "",
    points: 0,
    completed: false,
    notes: "",
  });
  let origin: HTMLElement | null = null;
  function edit(item?: Assignment, event?: Event) {
    origin = (event?.currentTarget as HTMLElement) || (document.activeElement as HTMLElement);
    fresh = !item;
    draft = item
      ? {
          $id: item.$id,
          courseCode: item.courseCode,
          title: item.title,
          dueDate: item.dueDate,
          category: item.category,
          points: item.points,
          completed: item.completed,
          notes: item.notes,
        }
      : {
          $id: "",
          courseCode: doc.current.courses[0]?.code || "",
          title: "",
          dueDate: day,
          category: "Homework",
          points: 0,
          completed: false,
          notes: "",
        };
    error = "";
    open = true;
  }
  function save(event: SubmitEvent) {
    event.preventDefault();
    error = "";
    if (!draft.title.trim()) {
      error = "Give this assignment a title.";
      return;
    }
    if (!Number.isFinite(draft.points) || draft.points < 0) {
      error = "Points must be zero or greater.";
      return;
    }
    const old = doc.current.assignments.find((item) => item.$id === draft.$id);
    if (draft.dueDate && !parseISO(draft.dueDate) && (!old || draft.dueDate !== old.dueDate)) {
      error = "Choose a valid due date or leave it undated.";
      return;
    }
    const title = draft.title.trim();
    if (fresh) {
      doc.fields.assignments.insert({
        courseCode: draft.courseCode,
        title,
        dueDate: draft.dueDate,
        category: draft.category,
        points: draft.points,
        completed: draft.completed,
        notes: draft.notes,
      });
    } else if (old) {
      doc.change((tx) => {
        const handle = tx.at(old);
        handle.courseCode.set(draft.courseCode);
        handle.title.replace(title);
        handle.dueDate.set(draft.dueDate);
        handle.category.replace(draft.category);
        handle.points.set(draft.points);
        handle.completed.set(draft.completed);
        handle.notes.replace(draft.notes);
      });
    }
    open = false;
  }
  function remove() {
    doc.fields.assignments.remove(draft.$id);
    open = false;
  }
  function restore(event: Event) {
    event.preventDefault();
    (origin?.isConnected ? origin : document.querySelector<HTMLElement>("[data-add-assignment]"))?.focus({ preventScroll: true });
  }
  const courseChoices = $derived.by(() => {
    const courses = doc.current.courses.map((course) => ({ code: course.code, name: course.name }));
    if (draft.courseCode && !courses.some((course) => course.code === draft.courseCode))
      courses.push({ code: draft.courseCode, name: "Saved course" });
    return courses;
  });
</script>

<Slop>
<main
  class={"asg-pad"}
  aria-label="Assignment tracker"
  data-slop-selection="none"
>
  <div class={"asg-app"}>
    <header class={"asg-header"}>
      <div class={"asg-identity"}>
        <span class={"asg-eyebrow"}>ASSIGNMENTS / STUDY PAD</span><input
          class={"asg-student"}
          aria-label="Student name"
          use:bindText={doc.fields.studentName}
          placeholder="Your study pad"
        /><input
          class={"asg-term"}
          aria-label="Term"
          use:bindText={doc.fields.term}
          placeholder="Your term"
        />
      </div>
      <div class={"asg-summary"}>
        <span class={"asg-readout"} data-urgent={overdue > 0 || dueToday > 0}
          >{readout}</span
        ><span class={"asg-summaryDetail"}
          >{overdue && dueToday
            ? `${dueToday} also due today`
            : `${completed.length} completed`}</span
        >
      </div>
    </header>
    <div class={"asg-toolbar"}>
      <Tabs.Root bind:value={filter}
        ><Tabs.List class={"asg-tabs"} aria-label="Assignment lists"
          ><Tabs.Trigger class={"asg-tab"} value="todo"
            >To do <span>{active.length}</span></Tabs.Trigger
          ><Tabs.Trigger class={"asg-tab"} value="done"
            >Done <span>{completed.length}</span></Tabs.Trigger
          ></Tabs.List
        ></Tabs.Root
      ><button
        class={"asg-add"}
        data-add-assignment
        onclick={(e) => edit(undefined, e)}
        ><Plus size={15} /> Add assignment</button
      >
    </div>
    <div class={"asg-stacks"}>
      {#each stacks as stack (stack.id)}<section
          class={"asg-stack"}
          aria-label={stack.label}
        >
          <div class={"asg-stackHead"}>
            <h2 class={"asg-stackLabel"} data-urgency={stack.id}>{stack.label}</h2>
            <span class={"asg-stackCount"}>{stack.items.length}</span>
          </div>
          <ul class={"asg-list"}>
            {#each stack.items as item (item.$id)}{@const due = relativeDue(
                item.dueDate,
                day,
              )}
              <li
                class={"asg-row"}
                data-done={item.completed}
                animate:flip={{
                  duration: prefersReducedMotion.current ? 0 : 180,
                }}
              >
                <Checkbox.Root
                  class={"asg-checkbox"}
                  checked={item.completed}
                  onCheckedChange={(checked) => doc.at(item).completed.set(checked === true)}
                  aria-label="Mark {item.title || 'assignment'} {item.completed
                    ? 'incomplete'
                    : 'complete'}"
                  >{#snippet children({ checked })}{#if checked}<Check
                        size={14}
                        strokeWidth={3}
                      />{/if}{/snippet}</Checkbox.Root
                >
                <button
                  class={"asg-itemBody"}
                  onclick={(e) => edit(item, e)}
                  aria-label="Edit {item.title || 'assignment'}"
                  ><strong class={"asg-itemTitle"}
                    >{item.title || "Untitled assignment"}</strong
                  ><span class={"asg-itemMeta"}
                    ><span
                      class={"asg-courseChip"}
                      title={doc.current.courses.find(
                        (c) => c.code === item.courseCode,
                      )?.name}>{item.courseCode || "No course"}</span
                    >{#if item.category}<span class={"asg-category"}
                        >{item.category}</span
                      >{/if}{#if item.points}<span>{item.points} pts</span
                      >{/if}</span
                  >{#if item.notes}<span class={"asg-notePreview"}
                      >{item.notes}</span
                    >{/if}</button
                >
                <div class={"asg-rowEnd"}>
                  <DuePicker
                    value={item.dueDate}
                    label={due.label}
                    urgency={item.completed ? "done" : due.urgency}
                    onSelect={(iso) => doc.at(item).dueDate.set(iso)}
                  /><button
                    class={"asg-editArrow"}
                    onclick={(e) => edit(item, e)}
                    aria-label="Assignment details for {item.title}"
                    ><ArrowUpRight size={15} /></button
                  >
                </div>
              </li>{/each}
          </ul>
        </section>
      {:else}<div class={"asg-empty"}>
          <span class={"asg-emptyCheck"}><Check size={30} /></span>
          <h2>
            {filter === "done"
              ? "Good work goes here."
              : doc.current.assignments.length
                ? "A little breathing room."
                : "Make room for good work."}
          </h2>
          <p>
            {filter === "done"
              ? "Check off an assignment when it’s finished."
              : doc.current.assignments.length
                ? "You’re caught up. Enjoy it."
                : "Add your first assignment and give it a due date."}
          </p>
          {#if filter !== "done"}<button
              class={"asg-secondary asg-primary"}
              onclick={(e) => edit(undefined, e)}>Add assignment</button
            >{/if}
        </div>{/each}
    </div>
    <footer class={"asg-footer"}>
      <span>One thing at a time.</span><span
        >{completed.length} / {doc.current.assignments.length} complete</span
      >
    </footer>
  </div>
</main>
<Dialog.Root bind:open
  ><Dialog.Portal
    ><Dialog.Overlay class={"asg-overlay"} /><Dialog.Content
      class={"asg-dialog"}
      onCloseAutoFocus={restore}
      ><Dialog.Title class={"asg-dialogTitle"}
        >{fresh ? "Add an assignment" : "Assignment details"}</Dialog.Title
      ><Dialog.Description class={"asg-description"}
        >What needs doing, and when?</Dialog.Description
      >
      <form class={"asg-form"} onsubmit={save}>
        <label class={"asg-formLabel"}
          >Assignment<textarea
            class={"asg-titleField asg-input"}
            bind:value={draft.title}
            placeholder="Lab, paper, or reading…"
            required
            rows="2"></textarea></label
        >
        <div class={"asg-formRow"}>
          <div class={"asg-formLabel"}>
            <span>Course</span><Select.Root
              type="single"
              bind:value={draft.courseCode}
              ><Select.Trigger class={"asg-input"} aria-label="Course"
                >{draft.courseCode || "No course"} ▾</Select.Trigger
              ><Select.Portal
                ><Select.Content class={"asg-selectContent"} sideOffset={5}
                  ><Select.Viewport
                    ><Select.Item
                      class={"asg-selectItem"}
                      value=""
                      label="No course">No course</Select.Item
                    >{#each courseChoices as c}<Select.Item
                        class={"asg-selectItem"}
                        value={c.code}
                        label={c.name}>{c.code} · {c.name}</Select.Item
                      >{/each}</Select.Viewport
                  ></Select.Content
                ></Select.Portal
              ></Select.Root
            >
          </div>
          <label class={"asg-formLabel"}
            >Category<input
              class={"asg-input"}
              bind:value={draft.category}
              placeholder="Essay, lab, reading…"
            /></label
          >
        </div>
        <div class={"asg-formRow"}>
          <div class={"asg-formLabel"}>
            <span>Due date</span><DuePicker
              value={draft.dueDate}
              label={relativeDue(draft.dueDate, day).label}
              urgency={relativeDue(draft.dueDate, day).urgency}
              onSelect={(iso) => (draft.dueDate = iso)}
            />{#if draft.dueDate}<button
                class={"asg-clearDate"}
                type="button"
                onclick={() => (draft.dueDate = "")}>Leave undated</button
              >{/if}
          </div>
          <label class={"asg-formLabel"}
            >Points<input
              class={"asg-input"}
              type="number"
              min="0"
              step="any"
              bind:value={draft.points}
            /></label
          >
        </div>
        <label class={"asg-formLabel"}
          >Notes<textarea
            class={"asg-notesField asg-input"}
            bind:value={draft.notes}
            placeholder="Instructions, links, or a place to start…"
            rows="4"></textarea></label
        >
        {#if error}<p class={"asg-error"} role="alert">{error}</p>{/if}
        <div class={"asg-actions"}>
          {#if !fresh}<button
              class={"asg-deleteButton asg-secondary asg-primary"}
              type="button"
              onclick={remove}>Delete</button
            >{/if}<Dialog.Close type="button" class={"asg-secondary asg-primary"}
            >Cancel</Dialog.Close
          ><button type="submit" class={"asg-primary"}>Save assignment</button>
        </div>
      </form></Dialog.Content
    ></Dialog.Portal
  ></Dialog.Root
>

{#snippet exportView()}
  <Export reference={day} />
{/snippet}
{#snippet icon()}
  <Icon />
{/snippet}
</Slop>
