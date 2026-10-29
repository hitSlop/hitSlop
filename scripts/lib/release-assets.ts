import { run } from "../../packages/tinys/src/cli/process";

// The original repository is renamed in place; its old URL remains a redirect.
export const releaseRepository = process.env.GITHUB_REPOSITORY?.toLowerCase() === "hitslop/tinys"
  ? "hitSlop/tinys" : "hitSlop/hitSlop";
export type Asset = { id: number; name: string; digest?: string };
export type PublishedRelease = { id: number; tag_name: string; draft: boolean; prerelease: boolean; immutable: boolean; assets: Asset[] };
export const stableTag = /^tinys-v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

export async function releaseInventory(): Promise<PublishedRelease[]> {
  const pages = JSON.parse(await run(["gh", "api", "--paginate", "--slurp", `repos/${releaseRepository}/releases?per_page=100`]));
  return pages.flat().filter((entry: PublishedRelease) => stableTag.test(entry.tag_name));
}

/** gh prints an LF status line, then headers and a blank line. Only the prefix is text. */
function assetResponse(bytes: Buffer): { status: number | undefined; body?: Buffer } {
  const firstLine = bytes.indexOf(10);
  const match = firstLine < 0 ? null
    : /^HTTP\/\d+(?:\.\d+)? (\d{3})(?: |\r?$)/.exec(bytes.subarray(0, firstLine).toString("ascii"));
  const status = match ? Number(match[1]) : undefined;
  if (status === undefined) return { status };
  for (let start = firstLine + 1; ;) {
    const end = bytes.indexOf(10, start);
    if (end < 0) return { status };
    if (end === start || (end === start + 1 && bytes[start] === 13))
      return { status, body: bytes.subarray(end + 1) };
    start = end + 1;
  }
}

export async function assetBytes(id: number): Promise<Buffer> {
  for (let attempt = 0; ; attempt++) {
    const child = Bun.spawn(["gh", "api", "--include", "-H", "Accept: application/octet-stream", `repos/${releaseRepository}/releases/assets/${id}`], { stdout: "pipe", stderr: "pipe" });
    const [bytes, error, code] = await Promise.all([new Response(child.stdout).arrayBuffer(), new Response(child.stderr).text(), child.exited]);
    const { status, body } = assetResponse(Buffer.from(bytes));
    if (!code) {
      if (status === undefined || body === undefined) throw new Error(`Invalid HTTP response for release asset ${id}`);
      if (status >= 200 && status < 300) return body;
    }
    const transient = status !== undefined && status >= 300
      ? status === 408 || status === 429 || (status >= 500 && status < 600)
      : /\bEOF\b|unexpected end of JSON input|(?:i\/o|TLS handshake) timeout|context deadline exceeded|connection reset|connection refused|temporary failure in name resolution|no such host|network is unreachable/i.test(error);
    const responseStatus = status === undefined ? "" : ` (HTTP ${status})`;
    if (!transient || attempt === 3)
      throw new Error(`Cannot download release asset ${id}${responseStatus}: ${error.trim() || `gh exited with status ${code}`}`);
    const delay = 1000 * 2 ** attempt;
    console.warn(`Release asset ${id}: transient GitHub download failure${responseStatus}; retry ${attempt + 1}/3 in ${delay / 1000}s.`);
    await Bun.sleep(delay);
  }
}

export function requiredAsset(release: PublishedRelease, name: string): Asset {
  const matches = release.assets.filter(asset => asset.name === name);
  if (matches.length !== 1) throw new Error(`${release.tag_name}: expected one ${name} asset`);
  return matches[0]!;
}
