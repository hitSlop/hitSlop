import { defineConfig } from "astro/config";
import svelte from "@astrojs/svelte";
import starlight from "@astrojs/starlight";
import sitemap from "@astrojs/sitemap";

export default defineConfig({
  site: "https://tinys.click",
  integrations: [
    svelte(),
    sitemap(),
    starlight({
      title: "tinys Docs",
      description: "Build small local-first apps and documents that live on your desktop.",
      favicon: "/favicon-32.png",
      disable404Route: true,
      customCss: ["./src/styles/docs.css"],
      components: {
        SiteTitle: "./src/components/DocsSiteTitle.astro",
        ThemeProvider: "./src/components/DocsThemeProvider.astro",
        ThemeSelect: "./src/components/DocsThemeSelect.astro",
      },
      editLink: {
        baseUrl: "https://github.com/hitSlop/tinys/edit/master/apps/landing/",
      },
      lastUpdated: true,
      social: [{ icon: "github", label: "GitHub", href: "https://github.com/hitSlop/tinys" }],
      sidebar: [
        {
          label: "Start here",
          items: [
            { label: "tinys documentation", slug: "docs" },
            { label: "Create your first tiny", slug: "docs/getting-started" },
          ],
        },
        {
          label: "Build a tiny",
          items: [
            { label: "Data and schemas", slug: "docs/guides/data-and-schemas" },
            { label: "Files and the web", slug: "docs/guides/files-and-web" },
            { label: "Capabilities and permissions", slug: "docs/guides/capabilities" },
            { label: "App definition and windows", slug: "docs/guides/manifest-and-windows" },
            { label: "Style a tiny", slug: "docs/guides/styling" },
            { label: "Where your tiny runs", slug: "docs/guides/where-your-tiny-runs" },
            { label: "Make it work on iPhone", slug: "docs/guides/mobile" },
            { label: "PNG window skins", slug: "docs/guides/png-window-skins" },
            { label: "Icons, widgets, previews, and exports", slug: "docs/guides/icons-and-exports" },
            { label: "Build, register, and share", slug: "docs/guides/build-and-share" },
          ],
        },
        {
          label: "Work with a document",
          items: [
            { label: "How .tiny files work", slug: "docs/concepts/how-tiny-files-work" },
            { label: "Edit installed documents", slug: "docs/guides/edit-installed-data" },
          ],
        },
        {
          label: "Reference",
          items: [
            { label: "CLI workflows", slug: "docs/guides/cli-workflows" },
            { label: "Availability", slug: "docs/availability" },
            { label: "Privacy", slug: "docs/privacy" },
            { label: "Live sharing", slug: "docs/guides/live-sharing" },
          ],
        },
      ],
    }),
  ],
  output: "static",
});
