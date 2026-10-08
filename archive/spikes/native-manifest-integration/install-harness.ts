import { readFile, writeFile, mkdir, copyFile } from "node:fs/promises";
import { resolve, join } from "node:path";
const workspace = resolve(import.meta.dir, "../../generated/native-manifest-integration/workspace");
const apple = join(workspace, "apps/apple/Packages/HitSlopApple");
const tests = join(apple, "Tests/HitSlopCoreTests/SlopPackageTests.swift");
const original = await readFile(tests, "utf8");
if (original.includes("nativeManifestParityRegressions"))
  throw new Error("Harness already installed");
await writeFile(
  tests,
  original + (await readFile(join(import.meta.dir, "regressions.swift"), "utf8")),
);
const packagePath = join(apple, "Package.swift");
const packageSource = (await readFile(packagePath, "utf8"))
  .replace(
    "products: [",
    'products: [\n    .executable(name: "manifest-integration-probe", targets: ["ManifestIntegrationProbe"]),',
  )
  .replace(
    "targets: [\n",
    'targets: [\n    .executableTarget(name: "ManifestIntegrationProbe", dependencies: ["HitSlopCore", "HitSlopCoreBinding"]),\n',
  );
await writeFile(packagePath, packageSource);
await mkdir(join(apple, "Sources/ManifestIntegrationProbe"), { recursive: true });
await copyFile(
  join(import.meta.dir, "probe.swift"),
  join(apple, "Sources/ManifestIntegrationProbe/main.swift"),
);
console.log("Installed shared baseline/candidate harness");
