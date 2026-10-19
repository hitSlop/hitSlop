/** Shared outputs, prepared once per build graph. No build-tool imports during selection. */
export const prerequisites = {
  engine: [],
  shell: [],
  native: ["engine"],
  sdk: [],
  browser: [],
  sharing: [],
  templates: ["engine", "shell"],
  packages: ["engine", "shell", "browser"],
  app: [],
} as const;
export type Build = keyof typeof prerequisites;
