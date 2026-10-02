import { stages, type Application } from "./schema";
import { daysFromToday } from "./dates";

export function stageCounts(applications: readonly Application[]): Record<(typeof stages)[number], number> {
  const counts = { wishlist: 0, applied: 0, assessment: 0, interview: 0, offer: 0, rejected: 0 };
  for (const app of applications) counts[app.stage] += 1;
  return counts;
}

// A follow-up is due once its date arrives, unless the application is settled.
export function needsNudge(app: Application): boolean {
  if (app.stage === "offer" || app.stage === "rejected" || !app.followUp) return false;
  return daysFromToday(app.followUp) <= 0;
}
