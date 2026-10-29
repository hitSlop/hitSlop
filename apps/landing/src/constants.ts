export const SITE_LINKS = {
  repository: "https://github.com/hitSlop/tinys",
  download: "https://github.com/hitSlop/tinys/releases/latest/download/tinys.dmg",
  // `bun run verify types` checks README.md and TinyLinks.swift match.
  iosDownload: "https://testflight.apple.com/join/F4qfCkgg",
  docs: "/docs/",
  authoring: "/docs/getting-started/",
  discord: "https://discord.gg/cqKRZjAWv3",
  x: "https://x.com/hitslop",
} as const;

// TODO: set to a form endpoint (e.g. a Worker route) to collect merch drop signups.
// While undefined, /merch links directly to Discord and collects no email.
export const NOTIFY_ENDPOINT: string | undefined = undefined;
