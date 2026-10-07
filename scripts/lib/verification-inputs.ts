/** Input ownership shared by local verification and CI scheduling. */
const rustInputs = [/^crates\//, /^Cargo\.(toml|lock)$/, /^\.cargo\//, /^rust-toolchain\.toml$/, /^rustfmt\.toml$/];
const nativeInputs = [
  ...rustInputs,
  /^apps\/apple\//,
  /^packages\/hitslop\/(src\/(sdk|shell|schema|wire)|generated|acceptance)\//,
  /^tests\/(abi|presentation|fixtures|compat)\//,
  /^examples\/slops\//,
  /^scripts\/(lib|build|templates)\//,
];

export const tierInputs = {
  hygiene: [/./],
  contracts: [
    ...rustInputs,
    /^packages\/hitslop\/(src\/(schema|wire)|generated|acceptance|tests\/schema)\//,
    /^packages\/hitslop\/(skills\/|src\/cli\/(skills-build|app)\.ts$)/,
    /^\.agents\/skills\//,
    /^scripts\/build\/(generate|rust-bindings|skills)\.ts$/,
    /^scripts\/build\/(acceptance|runner)\.ts$/,
    /^packages\/hitslop\/src\/(sdk|shell)\//,
    /\.generated\.(rs|swift)$/,
  ],
  types: [/\.(ts|svelte)$/, /(^|\/)tsconfig[^/]*\.json$/, /(^|\/)package\.json$/, /^bun\.lock$/],
  bun: [...rustInputs, /^packages\//, /^tests\/(examples|fixtures|compat|release)\//, /^scripts\//, /^examples\/slops\//],
  cli: [...rustInputs, /^packages\//, /^tests\/(fixtures|compat)\//, /^scripts\//, /^examples\/slops\//],
  rust: [...rustInputs, /^packages\/hitslop\/(src\/schema|generated|acceptance|tests\/schema)\//, /^tests\/compat\//, /^\.config\/nextest\.toml$/],
  landing: [/^apps\/landing\//],
  packed: [...rustInputs, /^examples\/slops\//, /^packages\/hitslop\/(src|templates|skills)\//, /^packages\/[^/]+\/package\.json$/, /^scripts\/(build|lib|templates)\//, /^tests\/packed\//],
  swift: nativeInputs,
  app: [],
  native: [...nativeInputs, /^packages\/hitslop\/(src\/cli|shell)\//, /^tests\/(native|examples)\//, /\.native\.test\.ts$/, /^scripts\/compat\//],
};

export const sharedInputs = [/^\.github\//, /^scripts\/lib\/(verification(?:-inputs)?|test-process|artifacts)\.ts$/, /^tests\/verification\//, /^scripts\/verify\.ts$/, /^package\.json$/, /^bun\.lock$/, /^rust-toolchain\.toml$/];

export type TierName = keyof typeof tierInputs;
export function affectedTiers(paths: string[], candidates: TierName[]) {
  return candidates.flatMap(name => {
    const hits = paths.filter(path => [...sharedInputs, ...tierInputs[name]].some(pattern => pattern.test(path)));
    return hits.length || name === "hygiene" ? [{ name, paths: hits }] : [];
  });
}
