import { prepareNativeFixtures, stageNativeFixtures } from "./native-fixtures";
import { useTestRegistry, repository } from "./runtime-artifacts";
useTestRegistry();
if (process.platform !== "darwin") throw new Error("Native tests require macOS.");
const fixtures = await prepareNativeFixtures();
// Benchmarks change the trial template's build stage before packing it.
if (Object.keys(process.env).some((name) => name.startsWith("HITSLOP_BENCH"))) await stageNativeFixtures();
const child = Bun.spawn(
  [
    "swift",
    "test",
    "--no-parallel",
    "--package-path",
    "apps/apple/Packages/HitSlopApple",
    ...process.argv.slice(2),
  ],
  {
    cwd: repository,
    env: { ...process.env, HITSLOP_PRESENTATION_FIXTURES: JSON.stringify(fixtures) },
    stdout: "inherit",
    stderr: "inherit",
  },
);
process.exit(await child.exited);
