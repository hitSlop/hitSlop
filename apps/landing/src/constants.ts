export const SITE_LINKS = {
  repository: "https://github.com/hitslop/hitslop",
  download: "https://github.com/hitslop/hitslop/releases/latest/download/hitSlop.dmg",
  // Temporary until the public TestFlight link; `bun run verify types` checks README.md and SlopLinks.swift match.
  iosDownload: "/docs/availability/#get-the-iphone-app",
  docs: "/docs/",
  authoring: "/docs/getting-started/",
  discord: "https://discord.gg/cqKRZjAWv3",
  x: "https://x.com/hitslop",
} as const;

// TODO: set to a form endpoint (e.g. a Worker route) to collect merch drop signups.
// While undefined, /merch links directly to Discord and collects no email.
export const NOTIFY_ENDPOINT: string | undefined = undefined;
