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
  for (let attempt = 0; ; attempt++) {
    const child = Bun.spawn(["gh", "api", "-H", "Accept: application/octet-stream", `repos/${releaseRepository}/releases/assets/${id}`], { stdout: "pipe", stderr: "pipe" });
    const [bytes, error, code] = await Promise.all([new Response(child.stdout).arrayBuffer(), new Response(child.stderr).text(), child.exited]);
    if (!code) return Buffer.from(bytes);
    const status = /\bHTTP (\d{3})\b/.exec(error)?.[1];
    const transient = status ? /^(408|429|5\d\d)$/.test(status)
      : /unexpected (EOF|end of JSON input)|(?:i\/o|TLS handshake) timeout|context deadline exceeded|connection reset|connection refused|temporary failure in name resolution|no such host|network is unreachable/i.test(error);
    if (!transient || attempt === 3) throw new Error(`Cannot download release asset ${id}: ${error}`);
    const delay = 1000 * 2 ** attempt;
    console.warn(`Release asset ${id}: transient GitHub download failure; retry ${attempt + 1}/3 in ${delay / 1000}s.`);
    await Bun.sleep(delay);
  }
}

export function requiredAsset(release: PublishedRelease, name: string): Asset {
  const matches = release.assets.filter(asset => asset.name === name);
  if (matches.length !== 1) throw new Error(`${release.tag_name}: expected one ${name} asset`);
  return matches[0]!;
}
