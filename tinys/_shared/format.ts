/** English labels shared by the editor and synchronous capture views. */
export type DateLabelOptions = {
  weekday?: "long" | "short" | "narrow";
  month?: "long" | "short" | "numeric" | "2-digit";
  day?: "numeric" | "2-digit";
  year?: "numeric";
  hour?: "numeric" | "2-digit";
  minute?: "2-digit";
  timeZone?: "UTC";
};
const months = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const weekdays = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const pad = (value: number) => String(value).padStart(2, "0");
export function dateLabel(value: Date | string | number, options: DateLabelOptions): string {
  const date = value instanceof Date ? value : new Date(typeof value === "string" && /^\d{4}-\d{2}-\d{2}$/.test(value) ? `${value}T12:00:00` : value);
  if (!Number.isFinite(date.getTime())) return "—";
  const utc = options.timeZone === "UTC";
  const month = utc ? date.getUTCMonth() : date.getMonth();
  const day = utc ? date.getUTCDate() : date.getDate();
  const year = utc ? date.getUTCFullYear() : date.getFullYear();
  const week = weekdays[utc ? date.getUTCDay() : date.getDay()]!;
  const parts: string[] = [];
  if (options.weekday) parts.push(options.weekday === "long" ? week : week.slice(0, options.weekday === "short" ? 3 : 1));
  const calendar: string[] = [];
  if (options.month) calendar.push(options.month === "long" ? months[month]! : options.month === "short" ? months[month]!.slice(0, 3) : options.month === "2-digit" ? pad(month + 1) : String(month + 1));
  if (options.day) calendar.push(options.day === "2-digit" ? pad(day) : String(day));
  if (calendar.length) parts.push(calendar.join(" "));
  if (options.year) parts.push(String(year));
  if (options.hour || options.minute) {
    const hour = utc ? date.getUTCHours() : date.getHours();
    const minute = utc ? date.getUTCMinutes() : date.getMinutes();
    parts.push(`${hour % 12 || 12}${options.minute ? `:${pad(minute)}` : ""} ${hour < 12 ? "AM" : "PM"}`);
  }
  return parts.join(", ");
}
export function numberLabel(value: number, maximumFractionDigits = 2, minimumFractionDigits = 0): string {
  if (!Number.isFinite(value)) return "—";
  let [whole, fraction = ""] = Math.abs(value).toFixed(maximumFractionDigits).split(".");
  while (fraction.length > minimumFractionDigits && fraction.endsWith("0")) fraction = fraction.slice(0, -1);
  whole = whole!.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  return `${value < 0 ? "−" : ""}${whole}${fraction ? `.${fraction}` : ""}`;
}
export function moneyLabel(value: number, currency: string, digits = currency === "JPY" ? 0 : 2): string {
  const symbols: Record<string, string> = { USD: "$", CAD: "CA$", EUR: "€", GBP: "£", JPY: "¥" };
  return `${symbols[currency] ?? `${currency} `}${numberLabel(value, digits, digits)}`;
}
