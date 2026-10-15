import { releases } from "./corpus";
import { verifyCorpus } from "./integrity";
import { resolveCorpora, verifyPreludes } from "./corpora";

if (import.meta.main) {
  if (!process.env.HITSLOP_COMPAT_ROOT) await resolveCorpora();
  const entries = await releases();
  if (!entries.length) throw new Error("No compatibility corpus to replay");
  for (const entry of entries) await verifyCorpus(entry.root, entry.release);
  await verifyPreludes(entries);
  console.log(`✓ ${entries.length} compatibility corpora verified`);
}
