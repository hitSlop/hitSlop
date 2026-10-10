/** Input ownership shared by local verification and CI scheduling. */
const rustBuildInputs = [/^Cargo\.(toml|lock)$/, /^\.cargo\//, /^rust-toolchain\.toml$/, /^rustfmt\.toml$/];
const rustInputs = [/^crates\//, ...rustBuildInputs];
const nativeInputs = [
  ...rustInputs,
  /^apps\/apple\//,
  /^packages\/hitslop\/(src\/(sdk|shell|schema|wire)|generated|acceptance)\//,
  /^tests\/(abi|apps|presentation|fixtures|compat)\//,
  /^scripts\/(lib|build|templates|compat)\//,
  /^\.github\/actions\/native-cache\//,
];

export const tierInputs = {
  compat: [/^tests\/compat\//, /^scripts\/compat\//, /^crates\/hitslop-core\/src\/(app|file)\//],
  tooling: [
    /^tests\/verification\//,
    /^scripts\/compat\/(check|corpus)\.ts$/,
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
  types: [/^examples\/slops\/(?!.*\.md$)/, /^tests\/apps\/(?!.*\.md$)/, /\.(ts|svelte)$/, /(^|\/)tsconfig[^/]*\.json$/, /(^|\/)package\.json$/, /^bun\.lock$/],
  bun: [...rustInputs, /^packages\/hitslop\/src\//, /^packages\/hitslop\/tests\/(sdk|shell)\/(?!.*\.browser\.test\.ts$)/, /^tests\/(fixtures|compat|release)\//, /^scripts\/(?!ci\/)/],
  cli: [...rustInputs, /^packages\/hitslop\/(src|templates|skills)\//, /^packages\/hitslop\/tests\/cli\/(?!.*\.(native|browser)\.test\.ts$)/, /^tests\/(apps|fixtures|compat)\//, /^scripts\/(?!ci\/)/],
  browser: [...rustInputs, /^packages\/hitslop\/(src|templates)\//, /\.browser\.test\.ts$/, /^packages\/hitslop\/tests\/cli\/.*-fixture\.ts$/, /^tests\/(browser|apps)\//, /^scripts\/(build|lib)\//],
  rust: [...rustInputs, /^packages\/hitslop\/(src\/schema|generated|acceptance|tests\/schema)\//, /^tests\/compat\//, /^\.config\/nextest\.toml$/],
  landing: [/^apps\/landing\//],
  packed: [...rustInputs, /^packages\/hitslop\/(src|templates|skills)\//, /^packages\/[^/]+\/package\.json$/, /^scripts\/(build|lib|templates)\//, /^tests\/packed\//],
  swift: nativeInputs,
  app: [],
  native: [...nativeInputs, /^packages\/hitslop\/(src\/cli|shell)\//, /^tests\/native\//, /\.native\.test\.ts$/, /^scripts\/compat\//],
};

/** On a pull request into master, the Swift and native tiers are required only when one of
 * these changes: what can break opening an existing document (the Rust core and FFI, the
 * native host, the page shell and wire types, the compatibility corpus) or the native checks
 * themselves. Compatibility-sensitive changes also open this gate. Other template and
 * CLI-only changes defer their Swift/native tiers until the master push. */
export const nativeGateInputs = [
  ...rustInputs,
  /^apps\/apple\//,
  /^packages\/hitslop\/(src\/(shell|wire)|generated|acceptance)\//,
  /^tests\/(abi|apps|compat|fixtures|native|presentation)\//,
  /\.native\.test\.ts$/,
  /^scripts\/(build|compat)\//,
  /^scripts\/lib\/(native|native-fixtures|swift-tests)\.ts$/,
  /^\.github\/actions\/native-cache\//,
];

// Policy workflows and tests of the runner do not change how product checks execute.
export const sharedInputs = [
  /^\.github\/workflows\/ci\.yml$/,
  /^\.github\/actions\/prepare-checks\//,
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
  /^crates\/[^/]+\/(?!tests\/|README\.md$)/,
  /^apps\/apple\/(?!.*\/Tests\/)/,
  /^apps\/apple\/.*\/Tests\/.*\/CompatCorpusTests\.swift$/,
  /^packages\/hitslop\/(src\/(sdk|shell|schema|wire)|generated|acceptance)\//,
  /^packages\/hitslop\/src\/cli\/(cli|main|documents|engine|process|json)\.ts$/,
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
