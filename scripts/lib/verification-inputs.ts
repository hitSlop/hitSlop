/** Input ownership shared by local verification and CI scheduling. */
const rustBuildInputs = [/^Cargo\.(toml|lock)$/, /^\.cargo\//, /^rust-toolchain\.toml$/, /^rustfmt\.toml$/];
const rustInputs = [/^tools\/(pop-bindgen|pop-compat)\//, /^crates\//, /^apps\/sharing\/src\//, /^apps\/sharing\/Cargo.toml$/, ...rustBuildInputs];
const nativeInputs = [
  ...rustInputs,
  /^apps\/apple\//,
  /^packages\/pop\/(src\/(sdk|shell|schema|wire)|generated|acceptance)\//,
  /^tests\/(abi|apps|presentation|fixtures|compat)\//,
  /^scripts\/(lib|build|templates|compat)\//,
  /^\.github\/actions\/native-cache\//,
];

export const appBuildInputs = [
  ...rustInputs,
  /^apps\/apple\//,
  /^packages\/pop\/(src\/(sdk|shell|schema|wire)|generated|acceptance|templates)\//,
  /^scripts\/(build|lib|templates|release)\//,
  /^pop\/(?!.*\.md$)/,
  /^\.github\/actions\/native-cache\//,
];

export const tierInputs = {
  compat: [/^tools\/pop-compat\//, /^tests\/compat\//, /^scripts\/compat\//, /^crates\/pop-core\/src\/app\//, /^crates\/pop-storage\/src\/file\//],
  tooling: [
    /^tests\/verification\//,
    /^scripts\/compat\//,
    /^scripts\/ci\//,
    /^\.github\/workflows\/(block-ai-attribution|secret-scan)\.yml$/,
    /^packages\/pop\/src\/cli\/process\.ts$/,
  ],
  contracts: [
    ...rustInputs,
    /^packages\/pop\/(src\/(schema|wire)|generated|acceptance|tests\/schema)\//,
    /^scripts\/build\/(generate|rust-bindings)\.ts$/,
    /^scripts\/build\/(acceptance|runner)\.ts$/,
    /^packages\/pop\/src\/(sdk|shell)\//,
    /\.generated\.(rs|swift)$/,
  ],
  // Also the README and the app's links, which scripts/ci/hygiene.ts compares with the site.
  types: [/^README\.md$/, /PopLinks\.swift$/, /^\.prettier(?:rc(?:\.[^/]*)?|ignore)$/, /^\.oxlintrc\.json$/, /^packages\/pop\/src\//, /^pop\/(?!.*\.md$)/, /^tests\/apps\/(?!.*\.md$)/, /\.(ts|svelte)$/, /(^|\/)tsconfig[^/]*\.json$/, /(^|\/)package\.json$/, /^bun\.lock$/],
  bun: [...rustInputs, /^packages\/pop\/src\//, /^packages\/pop\/tests\/(sdk|shell)\/(?!.*\.webkit\.test\.ts$)/, /^tests\/(fixtures|compat|release)\//, /^scripts\/(?!ci\/)/],
  cli: [...rustInputs, /^packages\/pop\/(src|templates|skills)\//, /^packages\/pop\/tests\/cli\/(?!.*\.(native|webkit)\.test\.ts$)/, /^tests\/(apps|fixtures|compat)\//, /^scripts\/(?!ci\/)/],
  render: [...rustInputs, /^packages\/pop\/(src|shell)\//, /^packages\/pop\/tests\/cli\/build\.native\.test\.ts$/, /^packages\/pop\/tests\/cli\/.*-fixture\.ts$/, /^tests\/native\/(render|landing)\.native\.test\.ts$/, /^apps\/landing\/scripts\/templates\.ts$/, /^tests\/(apps|fixtures|abi|presentation)\//, /^scripts\/(build|lib|templates)\//, /^pop\/bundled\.json$/, /^\.github\/actions\/native-cache\//],
  webkit: [...rustInputs, /^packages\/pop\/(src|templates)\//, /\.webkit\.test\.ts$/, /^packages\/pop\/tests\/cli\/.*-fixture\.ts$/, /^tests\/(webkit|apps)\//, /^scripts\/(build|lib)\//],
  rust: [/^scripts\/compat\//, ...rustInputs, /^packages\/pop\/(src\/schema|generated|acceptance|tests\/schema)\//, /^tests\/compat\//, /^\.config\/nextest\.toml$/],
  sharing: [/^\.github\/workflows\/sharing\.yml$/, ...rustInputs, /^apps\/sharing\//, /^scripts\/sharing\//, /^scripts\/build\/sharing\.ts$/, /^apps\/apple\/App\/Shared\/Assets\.xcassets\/AppIcon\.appiconset\/appicon-1024\.png$/],
  landing: [/^apps\/landing\//],
  packed: [...rustInputs, /^packages\/pop\/(src|templates|skills)\//, /^packages\/[^/]+\/package\.json$/, /^scripts\/(build|lib|templates)\//, /^tests\/packed\//],
  swift: nativeInputs,
  app: appBuildInputs,
  native: [...nativeInputs, /^packages\/pop\/(src\/cli|shell)\//, /^tests\/native\/(?!(render|landing)\.native\.test\.ts$)/, /^(?!packages\/pop\/tests\/cli\/build\.native\.test\.ts$|tests\/native\/(render|landing)\.native\.test\.ts$).*\.native\.test\.ts$/, /^scripts\/compat\//],
};

/** On a pull request into master, the Swift and native tiers are required only when one of
 * these changes: what can break opening an existing document (the Rust core and FFI, the
 * native host, the page shell and wire types, the compatibility corpus) or the native checks
 * themselves. Compatibility-sensitive changes also open this gate. Other template and
 * CLI-only changes defer their Swift/native tiers until the master push. */
export const nativeGateInputs = [
  ...rustInputs,
  /^apps\/apple\//,
  /^packages\/pop\/(src\/(shell|wire)|generated|acceptance)\//,
  /^tests\/(abi|apps|compat|fixtures|native|presentation)\//,
  /\.native\.test\.ts$/,
  /^scripts\/(build|compat)\//,
  /^scripts\/lib\/(native|native-fixtures|swift-tests|swift-format)\.ts$/,
  /^\.github\/actions\/native-cache\//,
];

/** On a pull request, the sharing tier (the built relay on Miniflare) is required only when one
 * of the tier's own inputs changes (the Rust workspace and its build inputs, so the core's sync
 * and document code and Cargo.lock included, the relay and its scripts). Master pushes and
 * release tags qualify it in sharing.yml. */
export const sharingGateInputs = tierInputs.sharing;

// Policy workflows and tests of the runner do not change how product checks execute.
export const sharedInputs = [
  /^\.github\/workflows\/ci\.yml$/,
  /^\.github\/actions\/prepare-checks\//,
  /^\.github\/actions\/select-xcode\//,
  /^scripts\/ci\/select\.ts$/,
  /^scripts\/lib\/(verification(?:-inputs)?|test-process|artifacts)\.ts$/,
  /^scripts\/verify\.ts$/,
  /^package\.json$/, /^bun\.lock$/, /^rust-toolchain\.toml$/,
];

/** These can change how saved documents are accepted, edited or rendered. Test-only
 * changes still open their owning gate, but need not replay every historical app. */
export const compatibilityInputs = [
  ...sharedInputs,
  ...rustBuildInputs,
  /^tools\/pop-compat\//,
  /^crates\/[^/]+\/(?!tests\/|README\.md$)/,
  /^apps\/apple\/(?!.*\/Tests\/)/,
  /^apps\/apple\/.*\/Tests\/.*\/CompatCorpusTests\.swift$/,
  /^packages\/pop\/(src\/(sdk|shell|schema|wire)|generated|acceptance)\//,
  /^packages\/pop\/src\/cli\/(cli|main|documents|engine|process|json)\.ts$/,
  /^tests\/(abi|compat|apps|fixtures)\//,
  /^tests\/native\/compat-replay\.native\.test\.ts$/,
  /^scripts\/(build|compat)\//,
  /^scripts\/lib\/(native|native-fixtures|swift-tests)\.ts$/,
];

export function touchesCompatibility(paths: string[]) {
  return paths.some(path => compatibilityInputs.some(pattern => pattern.test(path)));
}

export type TierName = keyof typeof tierInputs;
export function affectedTiers(paths: string[], candidates: TierName[]) {
  return candidates.flatMap(name => {
    const hits = paths.filter(path => [...sharedInputs, ...tierInputs[name]].some(pattern => pattern.test(path)));
    return hits.length ? [{ name, paths: hits }] : [];
  });
}
