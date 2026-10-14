import { attachments } from "hitslop/svelte";
import starterSheet from "./assets/bubu-spritesheet.webp";
import { parsePetArchive, validateSpriteImage, type PetMetadata } from "./pet-package";
import { STARTER, type StoredPackage } from "./pet";

/** A decoded spritesheet. It lives in the view that loaded it and is never saved. */
export type LoadedPet = {
  pet: PetMetadata;
  image: HTMLImageElement;
  /** Set when a stored package could not be shown and Bubu stands in. */
  error: string | null;
  /** Frees the object URL behind an imported sheet. */
  release: () => void;
};

async function decode(source: string): Promise<HTMLImageElement> {
  const image = new Image();
  image.decoding = "async";
  image.src = source;
  try { await image.decode(); }
  catch { throw new Error("The pet spritesheet could not be decoded."); }
  return image;
}

async function loadStarter(error: string | null = null): Promise<LoadedPet> {
  return { pet: STARTER, image: await decode(starterSheet), error, release: () => {} };
}

/** Loads the stored pet ZIP, or Bubu when none is stored or the stored one cannot be shown. */
export async function loadPet(stored: StoredPackage | undefined): Promise<LoadedPet> {
  if (!stored) return loadStarter();
  let url: string | null = null;
  try {
    const blob = await attachments.read(stored.id);
    const parsed = await parsePetArchive(new Uint8Array(await blob.arrayBuffer()));
    await validateSpriteImage(parsed);
    const bytes = new Uint8Array(parsed.spriteBytes.byteLength);
    bytes.set(parsed.spriteBytes);
    url = URL.createObjectURL(new Blob([bytes], { type: parsed.spriteMime }));
    const image = await decode(url);
    const release = () => URL.revokeObjectURL(image.src);
    return { pet: parsed, image, error: null, release };
  } catch (cause) {
    if (url) URL.revokeObjectURL(url);
    return loadStarter(`Stored pet unavailable: ${cause instanceof Error ? cause.message : String(cause)}`);
  }
}
