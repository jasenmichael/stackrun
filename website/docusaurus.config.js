import { readFileSync } from "node:fs";
import { join } from "node:path";
import { themes as prismThemes } from "prism-react-renderer";

const cargo = readFileSync(join(process.cwd(), "..", "Cargo.toml"), "utf8");
const version = cargo.match(/^version = "([^"]+)"/m)?.[1] ?? "0.0.0";

const siteUrl = "https://jasenmichael.github.io/stackrun/";
const siteDescription =
  "Run local commands in parallel. stackrun is a concurrently alternative with Cloudflare tunnels for stable HTTPS OAuth callback URLs.";

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: "stackrun",
  tagline:
    "Run commands in parallel — a concurrently alternative with Cloudflare tunnels for local OAuth",
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
  headTags: [
    {
      tagName: "script",
      attributes: { type: "application/ld+json" },
      innerHTML: JSON.stringify({
        "@context": "https://schema.org",
        "@type": "SoftwareApplication",
        name: "stackrun",
        applicationCategory: "DeveloperApplication",
        operatingSystem: "Linux, macOS, Windows",
        description: siteDescription,
        url: siteUrl,
        downloadUrl: "https://www.npmjs.com/package/stackrun",
        softwareVersion: version,
        license: "https://opensource.org/licenses/MIT",
        offers: {
          "@type": "Offer",
          price: "0",
          priceCurrency: "USD",
        },
      }),
    },
  ],
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
        sitemap: {
          changefreq: "weekly",
          priority: 0.5,
          filename: "sitemap.xml",
        },
        theme: {
          customCss: "./src/css/custom.css",
        },
      }),
    ],
  ],
  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      metadata: [
        { name: "description", content: siteDescription },
        {
          name: "keywords",
          content:
            "stackrun, concurrently alternative, run commands in parallel, Cloudflare tunnel, local OAuth, HTTPS callback, named tunnel, process orchestration, monorepo scripts",
        },
        { property: "og:title", content: "stackrun — run commands in parallel" },
        { property: "og:description", content: siteDescription },
        { property: "og:type", content: "website" },
        { property: "og:url", content: siteUrl },
        { name: "twitter:card", content: "summary" },
        {
          name: "twitter:title",
          content: "stackrun — run commands in parallel",
        },
        { name: "twitter:description", content: siteDescription },
      ],
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
