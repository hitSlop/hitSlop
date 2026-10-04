import { runCrashMatrix } from "./crash-matrix";
import { useTestRegistry } from "./runtime-artifacts";
useTestRegistry();
await runCrashMatrix(true);
