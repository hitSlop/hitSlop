<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import type { CaptureProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  import { gradebookView } from "./gradebook";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  const view = $derived(gradebookView(snapshot));
</script>

<main class="grade-app">
  <div class="grade-ledger">
    <header class="ledger-header">
      <div class="meta-block">
        <div class="header-label">ACADEMIC GRADEBOOK & FORECASTER</div>
        <div class="title-row">
          <h1 class="student-name">{snapshot.studentName}</h1>
          <span class="dot">·</span>
          <p class="term-name">{snapshot.term}</p>
        </div>
      </div>
      <div class="gpa-summary-card">
        <div class="gpa-badge"><span class="gpa-val">{view.termGPA}</span><span class="gpa-label">TERM GPA</span></div>
      </div>
    </header>
    {#each (preview ? (view.courses).slice(0, 3) : view.courses) as { course, metrics } (course.$id)}
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
            {#each (preview ? (course.categories).slice(0, 4) : course.categories) as category (category.$id)}
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
