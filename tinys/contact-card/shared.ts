import type { Tx, Value } from "@hitslop/tinys";
import type schema from "./schema";

export type Card = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

export const detailFields = ["name", "headline", "location", "bio", "email", "phone", "website"] as const;
export type DetailField = (typeof detailFields)[number];

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

/** The editor and the export derive their links, initials and channel list from one snapshot. */
export function cardView(data: Card) {
  const email = data.email.trim();
  return {
    tabLetter: indexLetter(data.name),
    monogram: initialsOf(data.name),
    websiteHref: hrefForWebsite(data.website),
    phoneHref: telHref(data.phone),
    mailHref: email ? `mailto:${email}` : "",
    hasChannel: Boolean(email || data.phone.trim() || data.website.trim()),
    portraitId: data.avatar?.id ?? "",
  };
}

function vCardEscape(value: string): string {
  return value.replace(/\\/g, "\\\\").replace(/\n/g, "\\n").replace(/,/g, "\\,").replace(/;/g, "\;");
}

export function vCardText(data: Card): string {
  const lines = ["BEGIN:VCARD", "VERSION:3.0", `FN:${vCardEscape(data.name.trim() || "Unnamed contact")}`];
  if (data.headline.trim()) lines.push(`TITLE:${vCardEscape(data.headline)}`);
  if (data.email.trim()) lines.push(`EMAIL;TYPE=INTERNET,WORK:${vCardEscape(data.email)}`);
  if (data.phone.trim()) lines.push(`TEL;TYPE=CELL:${vCardEscape(data.phone)}`);
  if (data.website.trim()) lines.push(`URL:${vCardEscape(hrefForWebsite(data.website) || data.website)}`);
  if (data.location.trim()) lines.push(`ADR;TYPE=WORK:;${vCardEscape(data.location)};;`);
  if (data.bio.trim()) lines.push(`NOTE:${vCardEscape(data.bio)}`);
  lines.push("END:VCARD");
  return lines.join("\r\n");
}

export function vCardFileName(data: Card): string {
  return `${(data.name.trim() || "contact").toLowerCase().replace(/\s+/g, "_")}.vcf`;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
const setters: { [K in DetailField]: (tx: Edit, value: string) => void } = {
  name: (tx, value) => tx.fields.name.set(value),
  headline: (tx, value) => tx.fields.headline.set(value),
  location: (tx, value) => tx.fields.location.set(value),
  bio: (tx, value) => tx.fields.bio.set(value),
  email: (tx, value) => tx.fields.email.set(value),
  phone: (tx, value) => tx.fields.phone.set(value),
  website: (tx, value) => tx.fields.website.set(value),
};
export function setField(tx: Edit, field: DetailField, value: string) {
  setters[field](tx, value);
}
export function setPortrait(tx: Edit, ref: { id: string; mimeType: string }) {
  tx.fields.avatar.$set({ id: ref.id, mimeType: ref.mimeType });
}
