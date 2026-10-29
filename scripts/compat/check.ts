import { releases } from "./corpus";
import { verifyCorpus } from "./integrity";
import { resolveCorpora, verifyPreludes, verifyResolvedCorpora } from "./corpora";

if (import.meta.main) {
  const resolved = process.env.TINYS_COMPAT_ROOT ? undefined : await resolveCorpora();
  await verifyResolvedCorpora(process.env.TINYS_COMPAT_ROOT!, { history: resolved?.entries });
  const entries = await releases();
  if (!entries.length) console.log("tinys has no published compatibility corpus yet; the first release must capture its baseline.");
  for (const entry of entries) await verifyCorpus(entry.root, entry.release);
  await verifyPreludes(entries);
  console.log(`✓ ${entries.length} compatibility corpora verified`);
}
