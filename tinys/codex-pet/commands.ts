import { defineCommands } from "@hitslop/tinys/commands";
import schema from "./schema";
import { petView, restoreStarter } from "./pet";

export default defineCommands(schema, {
  currentPet: {
    description: "Return the active pet's name, whether it came from an imported ZIP, and that ZIP's file name and size.",
    args: {},
    run: ({ current }) => {
      const view = petView(current);
      return {
        displayName: view.displayName,
        imported: view.imported,
        packageName: view.stored?.name ?? null,
        byteLength: view.stored?.byteLength ?? null,
      };
    },
  },
  restoreBubu: {
    description: "Forget the imported pet ZIP and bring back the bundled Bubu; refused when Bubu is already the pet.",
    args: {},
    run: ({ tx, current }) => restoreStarter(tx, current),
  },
});
