import { type CourseGrade } from "./schema";

export function percentToGradeLetter(pct: number): { letter: string; gpa: number; color: string } {
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

export function calculateCourseMetrics(course: CourseGrade) {
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
