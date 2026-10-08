/** Input ownership shared by local verification and CI scheduling. */
const rustInputs = [/^crates\//, /^Cargo\.(toml|lock)$/, /^\.cargo\//, /^rust-toolchain\.toml$/, /^rustfmt\.toml$/];
const nativeInputs = [
  ...rustInputs,
  /^apps\/apple\//,
  /^packages\/hitslop\/(src\/(sdk|shell|schema|wire)|generated|acceptance)\//,
  /^tests\/(abi|presentation|fixtures|compat)\//,
  /^examples\/slops\//,
  /^scripts\/(lib|build|templates)\//,
  /^\.github\/actions\/native-cache\//,
];

export const tierInputs = {
  compat: [/^tests\/compat\//, /^scripts\/compat\//, /^crates\/hitslop-core\/src\/(app|file)\//],
  tooling: [
    /^tests\/verification\//,
    /^scripts\/compat\/check\.ts$/,
    /^scripts\/ci\//,
    /^\.github\/workflows\/(block-ai-attribution|secret-scan)\.yml$/,
    /^packages\/hitslop\/src\/cli\/process\.ts$/,
  ],
  contracts: [
    ...rustInputs,
    /^packages\/hitslop\/(src\/(schema|wire)|generated|acceptance|tests\/schema)\//,
    /^scripts\/build\/(generate|rust-bindings)\.ts$/,
    /^scripts\/build\/(acceptance|runner)\.ts$/,
    /^packages\/hitslop\/src\/(sdk|shell)\//,
    /\.generated\.(rs|swift)$/,
  ],
  types: [/\.(ts|svelte)$/, /(^|\/)tsconfig[^/]*\.json$/, /(^|\/)package\.json$/, /^bun\.lock$/],
  bun: [...rustInputs, /^packages\//, /^tests\/(examples|fixtures|compat|release)\//, /^scripts\/(?!ci\/)/, /^examples\/slops\//],
  cli: [...rustInputs, /^packages\//, /^tests\/(fixtures|compat)\//, /^scripts\/(?!ci\/)/, /^examples\/slops\//],
  rust: [...rustInputs, /^packages\/hitslop\/(src\/schema|generated|acceptance|tests\/schema)\//, /^tests\/compat\//, /^\.config\/nextest\.toml$/],
  landing: [/^apps\/landing\//],
  packed: [...rustInputs, /^examples\/slops\//, /^packages\/hitslop\/(src|templates|skills)\//, /^packages\/[^/]+\/package\.json$/, /^scripts\/(build|lib|templates)\//, /^tests\/packed\//],
  swift: nativeInputs,
  app: [],
  native: [...nativeInputs, /^packages\/hitslop\/(src\/cli|shell)\//, /^tests\/(native|examples)\//, /\.native\.test\.ts$/, /^scripts\/compat\//, /^scripts\/dev\/live-sync\.ts$/],
};

// Policy workflows and tests of the runner do not change how product checks execute.
export const sharedInputs = [
  /^\.github\/workflows\/ci\.yml$/,
  /^\.github\/actions\/prepare-checks\//,
  /^scripts\/ci\/select\.ts$/,
  /^scripts\/lib\/(verification(?:-inputs)?|test-process|artifacts)\.ts$/,
  /^scripts\/verify\.ts$/,
  /^package\.json$/, /^bun\.lock$/, /^rust-toolchain\.toml$/,
];

export type TierName = keyof typeof tierInputs;
export function affectedTiers(paths: string[], candidates: TierName[]) {
  return candidates.flatMap(name => {
    const hits = paths.filter(path => [...sharedInputs, ...tierInputs[name]].some(pattern => pattern.test(path)));
    return hits.length ? [{ name, paths: hits }] : [];
  });
}
