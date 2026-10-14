import { refuse, type Tx, type Value } from "hitslop";
import type schema from "./schema";
import { calculateCourseMetrics } from "./shared";

export type Edit = Tx<typeof schema>;
export type Gradebook = Value<typeof schema.descriptor>;
type Course = Gradebook["courses"][number];

/** The page and Export derive their totals and course metrics from this one snapshot. */
export function gradebookView(data: Gradebook) {
  let totalQualityPoints = 0;
  let totalCredits = 0;
  const courses = data.courses.map((course) => {
    const metrics = calculateCourseMetrics(course);
    totalQualityPoints += metrics.currentGrade.gpa * course.credits;
    totalCredits += course.credits;
    return { course, metrics };
  });
  return {
    courses,
    termGPA: totalCredits > 0 ? (totalQualityPoints / totalCredits).toFixed(2) : "0.00",
    enrolledCredits: data.courses.reduce((sum, course) => sum + (course.credits || 0), 0),
  };
}

function findCourse(data: Gradebook, courseId: string): Course {
  const course = data.courses.find((row) => row.$id === courseId);
  if (!course) refuse("That course is no longer in the gradebook.");
  return course;
}

function findCategory(course: Course, categoryId: string) {
  const category = course.categories.find((row) => row.$id === categoryId);
  if (!category) refuse("That category is no longer in this course.");
  return category;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function insertCourse(tx: Edit) {
  return tx.fields.courses.insert({
    code: "NEW 101",
    name: "New Course",
    credits: 3,
    targetPercent: 90,
    categories: [
      { name: "Assignments", weightPercent: 40, scorePercent: 90 },
      { name: "Midterm Exam", weightPercent: 30, scorePercent: 85 },
      { name: "Final Exam", weightPercent: 30, isFinal: true },
    ],
  });
}

export function removeCourse(tx: Edit, data: Gradebook, courseId: string) {
  findCourse(data, courseId);
  tx.fields.courses.remove(courseId);
}

export function insertCategory(tx: Edit, data: Gradebook, courseId: string) {
  tx.at(findCourse(data, courseId)).categories.insert({ name: "New Category", weightPercent: 10 });
}

export function removeCategory(tx: Edit, data: Gradebook, courseId: string, categoryId: string) {
  const course = findCourse(data, courseId);
  tx.at(course).categories.remove(findCategory(course, categoryId).$id);
}

/** `score: null` clears the score, which leaves the category out of the average. */
export function setScore(tx: Edit, data: Gradebook, courseId: string, categoryId: string, score: number | null) {
  const category = findCategory(findCourse(data, courseId), categoryId);
  if (score === null) return tx.at(category).scorePercent.clear();
  if (!Number.isFinite(score)) refuse("Enter a number for the score.");
  tx.at(category).scorePercent.set(score);
}

export function setTarget(tx: Edit, data: Gradebook, courseId: string, target: number) {
  const course = findCourse(data, courseId);
  if (!Number.isFinite(target)) refuse("Enter a number for the target.");
  tx.at(course).targetPercent.set(target);
}

/** Exactly one category per course is the final exam; the solver reads it. */
export function setFinalCategory(tx: Edit, data: Gradebook, courseId: string, categoryId: string) {
  const course = findCourse(data, courseId);
  findCategory(course, categoryId);
  for (const category of course.categories) tx.at(category).isFinal.set(category.$id === categoryId);
}
