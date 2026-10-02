export function initialsOf(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (!parts.length) return "";
  const first = parts[0] ?? "";
  const last = parts.length > 1 ? parts[parts.length - 1] ?? "" : "";
  return last ? `${first[0] ?? ""}${last[0] ?? ""}`.toUpperCase() : first.slice(0, 2).toUpperCase();
}

export function hrefForWebsite(url: string): string {
  const value = url.trim();
  if (!value) return "";
  return /^https?:\/\//i.test(value) ? value : `https://${value}`;
}

export function telHref(phone: string): string {
  const value = phone.trim();
  if (!value) return "";
  return `tel:${value.replace(/[^\d+]/g, "")}`;
}

export function indexLetter(name: string): string {
  return (name.trim()[0] ?? "?").toUpperCase();
}
