import { defineConfig } from "astro/config";
import svelte from "@astrojs/svelte";
import starlight from "@astrojs/starlight";
import sitemap from "@astrojs/sitemap";

export default defineConfig({
  site: "https://hitslop.com",
  integrations: [
    svelte(),
    sitemap(),
    starlight({
      title: "hitSlop Docs",
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
        baseUrl: "https://github.com/hitSlop/hitslop/edit/master/apps/landing/",
      },
      lastUpdated: true,
      social: [{ icon: "github", label: "GitHub", href: "https://github.com/hitSlop/hitslop" }],
      sidebar: [
        {
          label: "Start here",
          items: [
            { label: "Overview", slug: "docs" },
            { label: "Create your first slop", slug: "docs/getting-started" },
          ],
        },
        {
          label: "Build a slop",
          items: [
            { label: "Data and schemas", slug: "docs/guides/data-and-schemas" },
            { label: "Manifest and windows", slug: "docs/guides/manifest-and-windows" },
            { label: "Style an app", slug: "docs/guides/styling" },
            { label: "PNG window skins", slug: "docs/guides/png-window-skins" },
            { label: "Icons, previews, and exports", slug: "docs/guides/icons-and-exports" },
            { label: "Build and share", slug: "docs/guides/build-and-share" },
          ],
        },
        {
          label: "Work with a document",
          items: [
            { label: "How .slop files work", slug: "docs/concepts/how-slop-files-work" },
            { label: "Edit installed data", slug: "docs/guides/edit-installed-data" },
          ],
        },
        {
          label: "Reference",
          items: [
            { label: "CLI workflows", slug: "docs/guides/cli-workflows" },
            { label: "Availability", slug: "docs/availability" },
            { label: "Collaboration (coming soon)", slug: "docs/guides/live-sharing" },
          ],
        },
      ],
    }),
  ],
  output: "static",
});
