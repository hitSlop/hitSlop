export function list(items: string[]) {
  return items.length < 3 ? items.join(" & ") : `${items.slice(0, -1).join(", ")} & ${items.at(-1)}`;
}
