export const SITE_LINKS = {
  repository: "https://github.com/hitslop/hitslop",
  download: "https://github.com/hitslop/hitslop/releases/latest/download/hitSlop.dmg",
  docs: "/docs/",
  authoring: "/docs/getting-started/",
  discord: "https://discord.gg/cqKRZjAWv3",
} as const;

// TODO: set to a form endpoint (e.g. a Worker route) to collect merch drop signups.
// While undefined, /merch shows "Signups opening soon" and collects no email.
export const NOTIFY_ENDPOINT: string | undefined = undefined;
