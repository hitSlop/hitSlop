# Codex Pet

A transparent desktop companion for Codex pet packages. Bubu is the bundled starter; drop a `.zip` with `pet.json` and `spritesheet.webp` to replace it.

From the repository root:

```sh
bun slop dev examples/slops/codex-pet
bun slop build examples/slops/codex-pet
bun slop register examples/slops/codex-pet
```

Create a writable copy of the registered template before editing.

Hover over the pet window to find more pets or import a ZIP. These controls follow
the native toolbar and hide after leaving, even if a button retains focus or an
import is running. Restore Bubu appears for imported pets.
Move the window using the native toolbar handle.

Bubu was created by Guo Beining (MIT): https://github.com/gbn666/codex-pets-bubuyier. The bundled notice is in `assets/BUBU-LICENSE.txt`.
