import { prepareNativeFixtures } from "./native-fixtures";
import { repository } from "./templates";
if (process.platform !== "darwin") throw new Error("Native tests require macOS.");
const fixtures = await prepareNativeFixtures();
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
