<script lang="ts">
  import doc from "./schema";
  import { calculateCourseMetrics, percentToGradeLetter } from "./shared";

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
</script>

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
