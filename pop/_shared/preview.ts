/** A short excerpt for artwork; the export always receives the complete value. */
export function previewText(value: string, preview: boolean, limit = 180): string {
  if (!preview) return value;
  const characters = Array.from(value);
  return characters.length > limit ? `${characters.slice(0, limit).join("").trimEnd()}…` : value;
}
