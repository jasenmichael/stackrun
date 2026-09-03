#!/usr/bin/env python3
"""Render README.md to the GitHub Pages index.html."""
from __future__ import annotations

import html
import re
import sys
from pathlib import Path

REPO = "https://github.com/jasenmichael/stackrun"
DOC_LINKS = {
    "SPEC.md": f"{REPO}/blob/main/SPEC.md",
    "STACK.md": f"{REPO}/blob/main/STACK.md",
    "DESIGN.md": f"{REPO}/blob/main/DESIGN.md",
    "PLAN.md": f"{REPO}/blob/main/PLAN.md",
    "ROADMAP.md": f"{REPO}/blob/main/ROADMAP.md",
    "./LICENSE": f"{REPO}/blob/main/LICENSE",
    "LICENSE": f"{REPO}/blob/main/LICENSE",
}


def inline(text: str) -> str:
    text = html.escape(text)
    text = re.sub(r"`([^`]+)`", r"<code>\1</code>", text)
    text = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", text)
    text = re.sub(r"(?<!\*)\*([^*]+)\*(?!\*)", r"<em>\1</em>", text)

    def img(m: re.Match[str]) -> str:
        alt, src = m.group(1), m.group(2)
        src = src.replace("docs/demo.svg", "demo.svg")
        src = src.replace("scripts/pages/demo.svg", "demo.svg")
        return f'<img alt="{alt}" src="{html.escape(src)}" />'

    text = re.sub(r"!\[([^\]]*)\]\(([^)]+)\)", img, text)

    def link(m: re.Match[str]) -> str:
        label, href = m.group(1), m.group(2)
        href = DOC_LINKS.get(href, href)
        return f'<a href="{html.escape(href)}">{label}</a>'

    text = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", link, text)
    return text


def slug(text: str) -> str:
    s = re.sub(r"<[^>]+>", "", text).lower()
    s = re.sub(r"[^a-z0-9]+", "-", s).strip("-")
    return s


def render_md(md: str) -> str:
    md = re.sub(r"<!--.*?-->", "", md, flags=re.S)
    lines = md.splitlines()
    out: list[str] = []
    i = 0
    para: list[str] = []
    demo_injected = False

    def flush_para() -> None:
        if para:
            out.append(f"<p>{inline(' '.join(para))}</p>")
            para.clear()

    while i < len(lines):
        line = lines[i]
        if line.startswith("```"):
            flush_para()
            lang = html.escape(line[3:].strip())
            i += 1
            body: list[str] = []
            while i < len(lines) and not lines[i].startswith("```"):
                body.append(lines[i])
                i += 1
            cls = f' class="language-{lang}"' if lang else ""
            out.append(f"<pre><code{cls}>{html.escape(chr(10).join(body))}\n</code></pre>")
            i += 1
            continue
        if re.match(r"^\|.+\|$", line) and i + 1 < len(lines) and re.match(
            r"^\|[\s:|-]+\|$", lines[i + 1]
        ):
            flush_para()
            headers = [c.strip() for c in line.strip("|").split("|")]
            i += 2
            rows: list[list[str]] = []
            while i < len(lines) and re.match(r"^\|.+\|$", lines[i]):
                rows.append([c.strip() for c in lines[i].strip("|").split("|")])
                i += 1
            thead = "".join(f"<th>{inline(h)}</th>" for h in headers)
            body_rows = []
            for row in rows:
                tds = "".join(f"<td>{inline(c)}</td>" for c in row)
                body_rows.append(f"<tr>{tds}</tr>")
            out.append(
                f"<table><thead><tr>{thead}</tr></thead><tbody>{''.join(body_rows)}</tbody></table>"
            )
            continue
        m = re.match(r"^(#{1,6})\s+(.*)$", line)
        if m:
            flush_para()
            level = len(m.group(1))
            title = inline(m.group(2).strip())
            hid = slug(m.group(2))
            out.append(f'<h{level} id="{hid}">{title}</h{level}>')
            i += 1
            continue
        if re.match(r"^[-*] ", line):
            flush_para()
            items: list[str] = []
            while i < len(lines) and re.match(r"^[-*] ", lines[i]):
                items.append(f"<li>{inline(lines[i][2:])}</li>")
                i += 1
            out.append(f"<ul>{''.join(items)}</ul>")
            continue
        if re.match(r"^\d+\. ", line):
            flush_para()
            items = []
            while i < len(lines) and re.match(r"^\d+\. ", lines[i]):
                items.append(f"<li>{inline(re.sub(r'^\d+\. ', '', lines[i]))}</li>")
                i += 1
            out.append(f"<ol>{''.join(items)}</ol>")
            continue
        if line.strip() == "---":
            flush_para()
            out.append("<hr />")
            i += 1
            continue
        if not line.strip():
            flush_para()
            i += 1
            continue
        para.append(line.strip())
        if "demo.svg" in line and not demo_injected:
            flush_para()
            out.append(
                '<div id="demo" class="cast" aria-label="stackrun asciinema demo"></div>'
            )
            demo_injected = True
        i += 1
    flush_para()
    if not demo_injected:
        out.append('<div id="demo" class="cast" aria-label="stackrun asciinema demo"></div>')
    return "\n".join(out)


def page(body: str, version: str) -> str:
    return f"""<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>stackrun</title>
    <meta
      name="description"
      content="Process-orchestration CLI. Alternative to concurrently, npm-run-all2, Wireit, and shell &amp;/wait/&amp;&amp;. v{html.escape(version)}"
    />
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/asciinema-player@3.8.2/dist/bundle/asciinema-player.css" />
    <style>
      :root {{
        color-scheme: dark light;
        --fg: #e8e6e3;
        --muted: #9a968f;
        --bg: #161513;
        --card: #221f1c;
        --accent: #7eb8da;
        --border: #3a3530;
      }}
      @media (prefers-color-scheme: light) {{
        :root {{
          --fg: #1a1816;
          --muted: #5c5852;
          --bg: #f6f3ee;
          --card: #fff;
          --accent: #0b6e99;
          --border: #ddd6cb;
        }}
      }}
      body {{
        margin: 0 auto;
        max-width: 46rem;
        padding: 2.5rem 1.25rem 4rem;
        font: 16px/1.55 ui-sans-serif, system-ui, sans-serif;
        color: var(--fg);
        background: var(--bg);
      }}
      h1, h2, h3 {{ font-weight: 650; letter-spacing: -0.02em; }}
      h1 {{ font-size: 2rem; }}
      a {{ color: var(--accent); }}
      code, pre {{ font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-size: 0.9em; }}
      pre {{
        background: var(--card);
        border: 1px solid var(--border);
        border-radius: 10px;
        padding: 0.9rem 1rem;
        overflow: auto;
      }}
      table {{ border-collapse: collapse; width: 100%; font-size: 0.95em; }}
      th, td {{ border-bottom: 1px solid var(--border); text-align: left; padding: 0.4rem 0.5rem; vertical-align: top; }}
      img {{ max-width: 100%; height: auto; border-radius: 10px; }}
      .cast {{ margin: 1rem 0 1.5rem; }}
      .meta {{ color: var(--muted); font-size: 0.9em; }}
    </style>
  </head>
  <body>
    {body}
    <p class="meta">stackrun v{html.escape(version)} · <a href="{REPO}">GitHub</a></p>
    <script src="https://cdn.jsdelivr.net/npm/asciinema-player@3.8.2/dist/bundle/asciinema-player.min.js"></script>
    <script>
      (function () {{
        var el = document.getElementById("demo");
        if (el && window.AsciinemaPlayer) {{
          AsciinemaPlayer.create("demo.cast", el, {{ preload: true, fit: "width" }});
        }}
      }})();
    </script>
  </body>
</html>
"""


def main() -> None:
    if len(sys.argv) != 4:
        sys.stderr.write("usage: render_readme.py README.md out.html version\n")
        sys.exit(2)
    readme = Path(sys.argv[1]).read_text(encoding="utf-8")
    version = sys.argv[3].lstrip("v")
    Path(sys.argv[2]).write_text(page(render_md(readme), version), encoding="utf-8")


if __name__ == "__main__":
    main()
