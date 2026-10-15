import { run } from "../../packages/hitslop/src/cli/process";

export const releaseRepository = "hitSlop/hitSlop";
export type Asset = { id: number; name: string; digest?: string };
export type PublishedRelease = { id: number; tag_name: string; draft: boolean; prerelease: boolean; immutable: boolean; assets: Asset[] };
export const stableTag = /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

export async function releaseInventory(): Promise<PublishedRelease[]> {
  const pages = JSON.parse(await run(["gh", "api", "--paginate", "--slurp", `repos/${releaseRepository}/releases?per_page=100`]));
  return pages.flat().filter((entry: PublishedRelease) => stableTag.test(entry.tag_name));
}

export async function assetBytes(id: number): Promise<Buffer> {
  const child = Bun.spawn(["gh", "api", "-H", "Accept: application/octet-stream", `repos/${releaseRepository}/releases/assets/${id}`], { stdout: "pipe", stderr: "pipe" });
  const [bytes, error, code] = await Promise.all([new Response(child.stdout).arrayBuffer(), new Response(child.stderr).text(), child.exited]);
  if (code) throw new Error(`Cannot download release asset ${id}: ${error}`);
  return Buffer.from(bytes);
}

export function requiredAsset(release: PublishedRelease, name: string): Asset {
  const matches = release.assets.filter(asset => asset.name === name);
  if (matches.length !== 1) throw new Error(`${release.tag_name}: expected one ${name} asset`);
  return matches[0]!;
}
