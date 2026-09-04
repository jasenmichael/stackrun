import { readFileSync } from "node:fs";
import { join } from "node:path";
import { themes as prismThemes } from "prism-react-renderer";

const cargo = readFileSync(join(process.cwd(), "..", "Cargo.toml"), "utf8");
const version = cargo.match(/^version = "([^"]+)"/m)?.[1] ?? "0.0.0";

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: "🥞 stackrun 🏃",
  tagline:
    "Process-orchestration CLI with Cloudflare tunnels for local stacks and auth callbacks",
  url: "https://jasenmichael.github.io",
  baseUrl: "/stackrun/",
  organizationName: "jasenmichael",
  projectName: "stackrun",
  deploymentBranch: "gh-pages",
  trailingSlash: false,
  onBrokenLinks: "throw",
  markdown: {
    hooks: {
      onBrokenMarkdownLinks: "warn",
    },
  },
  i18n: {
    defaultLocale: "en",
    locales: ["en"],
  },
  customFields: { version },
  presets: [
    [
      "classic",
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          routeBasePath: "/",
          sidebarPath: "./sidebars.js",
        },
        blog: false,
        theme: {
          customCss: "./src/css/custom.css",
        },
      }),
    ],
  ],
  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      navbar: {
        title: "🥞 stackrun 🏃",
        items: [
          {
            type: "docSidebar",
            sidebarId: "docs",
            position: "left",
            label: "Docs",
          },
          {
            href: "https://github.com/jasenmichael/stackrun",
            label: "GitHub",
            position: "right",
          },
        ],
      },
      footer: {
        style: "dark",
        links: [
          {
            title: "Docs",
            items: [
              {
                label: "Website",
                href: "https://jasenmichael.github.io/stackrun/",
              },
              {
                label: "Install",
                to: "/install",
              },
            ],
          },
          {
            title: "Project",
            items: [
              {
                label: "GitHub",
                href: "https://github.com/jasenmichael/stackrun",
              },
              {
                label: "npm",
                href: "https://www.npmjs.com/package/stackrun",
              },
            ],
          },
        ],
        copyright: `stackrun v${version} · MIT`,
      },
      prism: {
        theme: prismThemes.github,
        darkTheme: prismThemes.dracula,
        additionalLanguages: ["toml", "bash", "json"],
      },
    }),
};

export default config;
