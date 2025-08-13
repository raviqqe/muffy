import sitemap from "@astrojs/sitemap";
import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";

export default defineConfig({
  base: "/muffy",
  integrations: [
    sitemap(),
    starlight({
      customCss: ["./src/index.css"],
      favicon: "/icon.svg",
      head: [
        {
          attrs: {
            href: "/muffy/manifest.json",
            rel: "manifest",
          },
          tag: "link",
        },
        {
          attrs: {
            content: "/muffy/icon.svg",
            property: "og:image",
          },
          tag: "meta",
        },
      ],
      logo: {
        src: "./src/icon.svg",
      },
      sidebar: [
        {
          label: "Home",
          link: "/",
        },
        {
          label: "Install",
          link: "/install",
        },
        {
          label: "Usage",
          link: "/usage",
        },
      ],
      social: [
        {
          href: "https://github.com/raviqqe/muffy",
          icon: "github",
          label: "GitHub",
        },
      ],
      title: "Muffy",
    }),
  ],
  prefetch: { prefetchAll: true },
  site: "https://raviqqe.github.io/muffy",
});
