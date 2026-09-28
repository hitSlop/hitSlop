<script lang="ts">
  import { Slop, bindText, bindValue, useDocument } from "@hitslop/document/svelte";
  import Calculator from "@lucide/svelte/icons/calculator";
  import Check from "@lucide/svelte/icons/check";
  import Plus from "@lucide/svelte/icons/plus";
  import Target from "@lucide/svelte/icons/target";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import { Slider, Tabs } from "bits-ui";
  import schema, { type CourseGrade } from "./schema";

  const doc = useDocument(schema);
  let selectedCourseId = $state<string | null>(null);

  const selectedCourse = $derived(
    doc.current.courses.find((course) => course.$id === selectedCourseId) ?? doc.current.courses[0],
  );

  function percentToGradeLetter(pct: number): { letter: string; gpa: number; color: string } {
    if (pct >= 93) return { letter: "A", gpa: 4.0, color: "#2b593f" };
    if (pct >= 90) return { letter: "A-", gpa: 3.7, color: "#2b593f" };
    if (pct >= 87) return { letter: "B+", gpa: 3.3, color: "#2563eb" };
    if (pct >= 83) return { letter: "B", gpa: 3.0, color: "#2563eb" };
    if (pct >= 80) return { letter: "B-", gpa: 2.7, color: "#2563eb" };
    if (pct >= 77) return { letter: "C+", gpa: 2.3, color: "#d97706" };
    if (pct >= 73) return { letter: "C", gpa: 2.0, color: "#d97706" };
    if (pct >= 70) return { letter: "C-", gpa: 1.7, color: "#d97706" };
    if (pct >= 60) return { letter: "D", gpa: 1.0, color: "#ea580c" };
    return { letter: "F", gpa: 0.0, color: "#dc2626" };
  }

  function calculateCourseMetrics(course: CourseGrade) {
    let gradedWeight = 0;
    let earnedWeight = 0;
    const finalCategory = course.categories.find((category) => category.isFinal);

    for (const category of course.categories) {
      if (typeof category.scorePercent === "number") {
        gradedWeight += category.weightPercent;
        earnedWeight += (category.weightPercent * category.scorePercent) / 100;
      }
    }

    const currentAverage = gradedWeight > 0 ? (earnedWeight / gradedWeight) * 100 : 0;
    const currentGrade = percentToGradeLetter(currentAverage);

    let neededOnFinal: number | null = null;
    let solverStatus: "possible" | "locked" | "impossible" | "no-final" = "possible";

    if (finalCategory && finalCategory.weightPercent > 0) {
      const remainingTargetPoints = course.targetPercent - earnedWeight;
      neededOnFinal = (remainingTargetPoints / finalCategory.weightPercent) * 100;
      if (neededOnFinal <= 0) solverStatus = "locked";
      else if (neededOnFinal > 100) solverStatus = "impossible";
      else solverStatus = "possible";
    } else {
      solverStatus = "no-final";
    }

    return { gradedWeight, earnedWeight, currentAverage, currentGrade, finalCategory, neededOnFinal, solverStatus };
  }

  const selectedMetrics = $derived(selectedCourse ? calculateCourseMetrics(selectedCourse) : null);

  const overallTermGPA = $derived.by(() => {
    let totalQualityPoints = 0;
    let totalCredits = 0;
    for (const course of doc.current.courses) {
      const metrics = calculateCourseMetrics(course);
      totalQualityPoints += metrics.currentGrade.gpa * course.credits;
      totalCredits += course.credits;
    }
    return totalCredits > 0 ? (totalQualityPoints / totalCredits).toFixed(2) : "0.00";
  });

  const totalCredits = $derived(doc.current.courses.reduce((sum, course) => sum + (course.credits || 0), 0));

  function addCourse() {
    const inserted = doc.fields.courses.insert({
      code: "NEW 101",
      name: "New Course",
      credits: 3,
      targetPercent: 90,
      categories: [
        { name: "Assignments", weightPercent: 40, scorePercent: 90, isFinal: false },
        { name: "Midterm Exam", weightPercent: 30, scorePercent: 85, isFinal: false },
        { name: "Final Exam", weightPercent: 30, isFinal: true },
      ],
    });
    selectedCourseId = inserted.id;
  }

  function removeCourse(id: string) {
    doc.fields.courses.remove(id);
    if (selectedCourseId === id) selectedCourseId = doc.current.courses[0]?.$id ?? null;
  }

  function addCategory() {
    const course = selectedCourse;
    if (!course) return;
    doc.at(course).categories.insert({ name: "New Category", weightPercent: 10, isFinal: false });
  }

  function removeCategory(catId: string) {
    const course = selectedCourse;
    if (!course) return;
    doc.at(course).categories.remove(catId);
  }

  function setFinalCategory(catId: string) {
    const course = selectedCourse;
    if (!course) return;
    doc.change((tx) => {
      for (const category of course.categories) tx.at(category).isFinal.set(category.$id === catId);
    });
  }

  function setScore(category: CourseGrade["categories"][number], raw: string) {
    const handle = doc.at(category).scorePercent;
    if (raw.trim() === "") {
      handle.clear();
      return;
    }
    const value = Number(raw);
    if (Number.isFinite(value)) handle.set(value);
  }
</script>

<Slop>
<main class="grade-app">
  <div class="grade-ledger">
    <!-- Header -->
    <header class="ledger-header">
      <div class="meta-block">
        <div class="header-label">ACADEMIC GRADEBOOK & FORECASTER</div>
        <div class="title-row">
          <input class="student-name" use:bindText={doc.fields.studentName} placeholder="Student Name" />
          <span class="dot">·</span>
          <input class="term-name" use:bindText={doc.fields.term} placeholder="Academic Term" />
        </div>
      </div>

      <!-- Cumulative GPA Pill -->
      <div class="gpa-summary-card">
        <div class="gpa-badge">
          <span class="gpa-val">{overallTermGPA}</span>
          <span class="gpa-label">TERM GPA</span>
        </div>
        <div class="gpa-meta">
          <span class="gpa-credits">{totalCredits} Enrolled Credits</span>
          <span class="gpa-status">Good Standing</span>
        </div>
      </div>
    </header>

    <!-- Main Workspace -->
    <div class="ledger-body">
      <!-- Course Selector Sidebar -->
      <aside class="courses-sidebar">
        <div class="sidebar-head">
          <span class="sidebar-title">COURSES</span>
          <button
            type="button"
            class="add-course-btn"
            data-slop-export="hide"
            onclick={addCourse}
            title="Add new course"
          >
            <Plus size={12} />
          </button>
        </div>

        <Tabs.Root
          value={selectedCourse?.$id ?? ""}
          onValueChange={(v) => { if (v) selectedCourseId = v; }}
        >
          <Tabs.List class="courses-nav" aria-label="Courses">
            {#each doc.current.courses as course (course.$id)}
              {@const m = calculateCourseMetrics(course)}
              <Tabs.Trigger
                value={course.$id}
                class="course-nav-item {course.$id === selectedCourse?.$id ? 'active' : ''}"
              >
                <div class="course-nav-left">
                  <span class="course-code">{course.code}</span>
                  <span class="course-title">{course.name}</span>
                </div>
                <div class="course-nav-right">
                  <span class="grade-badge" style="background-color: {m.currentGrade.color};">
                    {m.currentGrade.letter}
                  </span>
                  <span class="avg-pct">{m.currentAverage.toFixed(1)}%</span>
                </div>
              </Tabs.Trigger>
            {/each}
          </Tabs.List>
        </Tabs.Root>
      </aside>

      <!-- Course Detail & Target Solver -->
      {#if selectedCourse && selectedMetrics}
        <section class="course-detail-view">
          <!-- Course Subheader -->
          <div class="detail-header">
            <div class="course-info">
              <input class="course-code-input" use:bindText={doc.at(selectedCourse).code} />
              <span class="sep">·</span>
              <input class="course-name-input" use:bindText={doc.at(selectedCourse).name} />
              <div class="credits-wrap">
                <input class="credits-input" type="number" step="0.5" use:bindValue={doc.at(selectedCourse).credits} />
                <span class="credits-lbl">cr</span>
              </div>
            </div>

            <button
              type="button"
              class="del-course-btn"
              data-slop-export="hide"
              onclick={() => removeCourse(selectedCourse.$id)}
              title="Delete course"
            >
              <Trash2 size={12} />
            </button>
          </div>

          <!-- Weights Table -->
          <div class="weights-table-container">
            <table class="weights-table">
              <thead>
                <tr>
                  <th>Category</th>
                  <th style="width: 80px;">Weight %</th>
                  <th style="width: 90px;">Score %</th>
                  <th style="width: 85px;">Weighted</th>
                  <th style="width: 70px;">Final Exam</th>
                  <th style="width: 30px;" data-slop-export="hide"></th>
                </tr>
              </thead>
              <tbody>
                {#each selectedCourse.categories as cat (cat.$id)}
                  {@const weighted = typeof cat.scorePercent === "number" ? ((cat.weightPercent * cat.scorePercent) / 100).toFixed(1) : "—"}
                  <tr>
                    <td>
                      <input class="table-input" use:bindText={doc.at(cat).name} placeholder="Category" />
                    </td>
                    <td>
                      <div class="num-unit">
                        <input class="table-num" type="number" use:bindValue={doc.at(cat).weightPercent} min="0" max="100" />
                        <span>%</span>
                      </div>
                    </td>
                    <td>
                      <div class="num-unit">
                        <input
                          class="table-num"
                          type="number"
                          placeholder="Pending"
                          value={cat.scorePercent ?? ""}
                          oninput={(e) => setScore(cat, e.currentTarget.value)}
                          min="0"
                          max="150"
                        />
                        <span>%</span>
                      </div>
                    </td>
                    <td class="weighted-col">
                      <span>{weighted}%</span>
                    </td>
                    <td class="final-toggle-col">
                      <button
                        type="button"
                        class="final-chip"
                        class:is-final={cat.isFinal}
                        data-slop-export="hide"
                        onclick={() => setFinalCategory(cat.$id)}
                        title={cat.isFinal ? "Designated final exam" : "Click to set as final exam"}
                      >
                        {cat.isFinal ? "Final" : "—"}
                      </button>
                    </td>
                    <td data-slop-export="hide">
                      <button
                        type="button"
                        class="cat-del-btn"
                        onclick={() => removeCategory(cat.$id)}
                        title="Delete category"
                      >
                        <Trash2 size={11} />
                      </button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>

            <div class="add-cat-row" data-slop-export="hide">
              <button type="button" class="add-cat-btn" onclick={addCategory}>
                <Plus size={11} />
                <span>Add Syllabus Category</span>
              </button>
            </div>
          </div>

          <!-- Solver Card: What do I need on the final? -->
          <div class="solver-card">
            <div class="solver-top">
              <div class="solver-title-wrap">
                <Target size={14} class="solver-icon" />
                <span class="solver-title">WHAT DO I NEED ON THE FINAL?</span>
              </div>

              <!-- Target Slider -->
              <div class="target-controller">
                <span class="target-lbl">Target Grade:</span>
                <Slider.Root
                  type="single"
                  value={selectedCourse.targetPercent} onValueChange={(value) => doc.at(selectedCourse).targetPercent.set(value)}
                  min={70}
                  max={98}
                  step={1}
                  class="target-slider-root"
                  aria-label="Target Grade"
                >
                  {#snippet children({ thumbs })}
                    <span class="target-slider-track">
                      <Slider.Range class="target-slider-range" />
                    </span>
                    {#each thumbs as index}
                      <Slider.Thumb {index} class="target-slider-thumb" aria-label="Target percent" />
                    {/each}
                  {/snippet}
                </Slider.Root>
                <span class="target-val">{selectedCourse.targetPercent}% ({percentToGradeLetter(selectedCourse.targetPercent).letter})</span>
              </div>
            </div>

            <div class="solver-result-banner">
              {#if selectedMetrics.solverStatus === "locked"}
                <div class="result-box success">
                  <Check size={16} />
                  <span><strong>Locked in!</strong> You have already secured your target {percentToGradeLetter(selectedCourse.targetPercent).letter} even with 0% on the final.</span>
                </div>
              {:else if selectedMetrics.solverStatus === "impossible"}
                <div class="result-box alert">
                  <span>Target requires <strong>{selectedMetrics.neededOnFinal?.toFixed(1)}%</strong> on the final exam ({selectedMetrics.finalCategory?.name}). Extra credit required.</span>
                </div>
              {:else if selectedMetrics.solverStatus === "possible"}
                <div class="result-box target">
                  <Calculator size={16} />
                  <span>You need <strong>{selectedMetrics.neededOnFinal?.toFixed(1)}%</strong> on the {selectedMetrics.finalCategory?.name || "Final Exam"} to earn an <strong>{percentToGradeLetter(selectedCourse.targetPercent).letter} ({selectedCourse.targetPercent}%)</strong> in this course.</span>
                </div>
              {:else}
                <div class="result-box muted">
                  <span>Mark one category above as "Final Exam" to enable the exam target solver.</span>
                </div>
              {/if}
            </div>
          </div>
        </section>
      {/if}
    </div>
  </div>
</main>

{#snippet exportView()}
  <main class="grade-app">
    <div class="grade-ledger">
      <header class="ledger-header">
        <div class="meta-block">
          <div class="header-label">ACADEMIC GRADEBOOK & FORECASTER</div>
          <div class="title-row">
            <h1 class="student-name">{doc.current.studentName}</h1>
            <span class="dot">·</span>
            <p class="term-name">{doc.current.term}</p>
          </div>
        </div>
        <div class="gpa-summary-card">
          <div class="gpa-badge"><span class="gpa-val">{overallTermGPA}</span><span class="gpa-label">TERM GPA</span></div>
        </div>
      </header>
      {#each doc.current.courses as course (course.$id)}
        {@const metrics = calculateCourseMetrics(course)}
        <section class="course-detail-view">
          <div class="detail-header">
            <div class="course-info">
              <span class="course-code-input">{course.code}</span>
              <span class="sep">·</span>
              <span class="course-name-input">{course.name}</span>
              <span class="credits-lbl">{course.credits} cr · {metrics.currentGrade.letter} {metrics.currentAverage.toFixed(1)}%</span>
            </div>
          </div>
          <table class="weights-table">
            <tbody>
              {#each course.categories as category (category.$id)}
                <tr>
                  <td>{category.name}</td>
                  <td>{category.weightPercent}%</td>
                  <td>{typeof category.scorePercent === "number" ? category.scorePercent : "Pending"}</td>
                  <td>{category.isFinal ? "Final" : ""}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </section>
      {/each}
    </div>
  </main>
{/snippet}

{#snippet icon()}
<section class="grade-render grade-icon" data-slop-render="icon" aria-hidden="true">
  <div class="icon-instrument">
    <div class="icon-header">
      <span class="icon-title-bar"></span>
      <div class="icon-gpa-dial">
        <span class="icon-gpa-text"></span>
      </div>
    </div>
    <div class="icon-courses-list">
      <div class="icon-course-row">
        <span class="icon-grade-tag grade-a"></span>
        <span class="icon-bar"></span>
        <span class="icon-pct-bar"></span>
      </div>
      <div class="icon-course-row">
        <span class="icon-grade-tag grade-a"></span>
        <span class="icon-bar mid"></span>
        <span class="icon-pct-bar"></span>
      </div>
      <div class="icon-course-row">
        <span class="icon-grade-tag grade-b"></span>
        <span class="icon-bar"></span>
        <span class="icon-pct-bar"></span>
      </div>
      <div class="icon-course-row">
        <span class="icon-grade-tag grade-a"></span>
        <span class="icon-bar short"></span>
        <span class="icon-pct-bar"></span>
      </div>
    </div>
    <div class="icon-solver-panel">
      <span class="icon-solver-head"></span>
      <span class="icon-solver-track"></span>
    </div>
  </div>
</section>
{/snippet}
</Slop>
