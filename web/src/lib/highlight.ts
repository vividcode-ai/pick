import { codeToHtml } from "shiki";

const EXT_TO_LANG: Record<string, string> = {
  ts: "typescript", tsx: "tsx", js: "javascript", jsx: "jsx", mjs: "javascript", cjs: "javascript",
  rs: "rust", py: "python", go: "go", zig: "zig",
  css: "css", scss: "scss", less: "less",
  json: "json", jsonc: "jsonc", json5: "json5",
  md: "markdown", mdx: "mdx",
  html: "html", htm: "html", xml: "xml", xhtml: "html", svg: "xml",
  yaml: "yaml", yml: "yaml", toml: "toml",
  sh: "shellscript", bash: "shellscript", zsh: "shellscript",
  ps1: "powershell", psd1: "powershell", psm1: "powershell",
  sql: "sql", graphql: "graphql", gql: "graphql",
  dockerfile: "dockerfile",
  makefile: "makefile", mk: "makefile",
  c: "c", cpp: "cpp", h: "c", hpp: "cpp", cc: "cpp", cxx: "cpp",
  java: "java", kotlin: "kt" as string, kts: "kotlin",
  swift: "swift", rb: "ruby", php: "php", r: "r",
  lua: "lua", dart: "dart",
  svelte: "svelte", vue: "vue", astro: "astro",
  tex: "latex", bib: "bibtex",
  diff: "diff", patch: "diff",
  prisma: "prisma",
  solidity: "solidity",
  terraform: "terraform", tf: "terraform",
  nix: "nix",
  cmake: "cmake", cmake_in: "cmake",
  elisp: "elisp", clj: "clojure",
  erl: "erlang", hrl: "erlang",
  ex: "elixir", exs: "elixir",
  fs: "fsharp", fsx: "fsharp",
  hs: "haskell", lhs: "haskell",
  ml: "ocaml", mli: "ocaml",
  pas: "pascal", pp: "pascal",
  pl: "perl", pm: "perl",
  scala: "scala", sc: "scala",
  cs: "csharp", csx: "csharp",
  fsproj: "xml", csproj: "xml",
  bat: "bat", cmd: "bat",
  ini: "ini", cfg: "ini", conf: "ini",
  env: "dotenv",
  editorconfig: "editorconfig",
  gitignore: "gitignore",
  lockfile: "text",
  "": "text",
};

const THEME = "github-dark-dimmed";

let htmlCache = new Map<string, string>();

export async function highlightCode(code: string, filePath: string): Promise<string> {
  const ext = filePath.split(".").pop()?.toLowerCase() || "";
  const lang = EXT_TO_LANG[ext] || ext;

  const cacheKey = `${lang}:${code.length}:${code.slice(0, 100)}`;
  const cached = htmlCache.get(cacheKey);
  if (cached) return cached;

  try {
    let html = await codeToHtml(code, { lang, theme: THEME });

    let lineNum = 0;
    html = html.replace(
      /<span class="line">/g,
      () => {
        lineNum++;
        return `<div class="line-wrapper" data-line="${lineNum}"><span class="line"><span class="line-num">${lineNum}</span>`;
      }
    );

    html = html.replace(
      /<\/span>\n(?=<div class="line-wrapper")/g,
      '<button class="line-add-btn">+</button></div>\n'
    );

    html = html.replace(
      /<\/span>(<\/code>)/g,
      '<button class="line-add-btn">+</button></div>$1'
    );

    htmlCache.set(cacheKey, html);
    if (htmlCache.size > 50) {
      const firstKey = htmlCache.keys().next().value;
      if (firstKey) htmlCache.delete(firstKey);
    }
    return html;
  } catch {
    let fallback = `<pre class="shiki" style="padding:0;font-size:12px;line-height:1.5;overflow:auto;background:transparent;"><code>${escapeHtml(code)}</code></pre>`;
    let lineNum = 0;
    fallback = fallback.replace(
      /<span class="line">/g,
      () => {
        lineNum++;
        return `<div class="line-wrapper" data-line="${lineNum}"><span class="line"><span class="line-num">${lineNum}</span>`;
      }
    );
    fallback = fallback.replace(
      /<\/span>\n(?=<div class="line-wrapper")/g,
      '<button class="line-add-btn">+</button></div>\n'
    );
    fallback = fallback.replace(
      /<\/span>(<\/code>)/g,
      '<button class="line-add-btn">+</button></div>$1'
    );
    return fallback;
  }
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/\n/g, "</span>\n<span class=\"line\">");
}
