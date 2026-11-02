import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { legacyAppcast, verifyAppcasts } from "./record";

const [directory, build] = process.argv.slice(2);
if (!directory || !/^\d+$/.test(build ?? "")) throw new Error("Expected output directory and Mac build number");
await writeFile(join(directory, "appcast.xml"), legacyAppcast);
await verifyAppcasts(directory, build!);
