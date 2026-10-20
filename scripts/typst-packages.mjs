#!/usr/bin/env node
// Every Typst package on a package path, as the files a compile in the tab is
// handed: one JSON object from the key typst-wasm finds a file under
// (`@local/typst-design:0.1.0/lib.typ`) to its text. The tab's kit and the
// box's typst read the same package path (flake.nix, `typstPackages`), so the
// two compile against one package.
//
//   node scripts/typst-packages.mjs [<package path>]   default $TYPST_PACKAGE_PATH
//
// A package's own `exclude` (its typst.toml) is honoured as typst's packaging
// does, by path or by the directory a path starts with; only text a compile
// can read is kept (`.typ` and the manifest).
import { readFileSync, readdirSync, statSync } from "node:fs"
import { join, relative, sep } from "node:path"

const root = process.argv[2] ?? process.env.TYPST_PACKAGE_PATH
if (root === undefined || root === "") {
  console.error("typst-packages.mjs: no package path given and TYPST_PACKAGE_PATH is unset (enter nix develop .#triage)")
  process.exit(1)
}

const dirs = (p) => readdirSync(p).filter((n) => statSync(join(p, n)).isDirectory())

const walk = (dir) =>
  readdirSync(dir).flatMap((n) => {
    const p = join(dir, n)
    return statSync(p).isDirectory() ? walk(p) : [p]
  })

// The manifest's `exclude = [..]`, a list of plain strings in every package this reads.
const excluded = (manifest) => {
  const m = /^exclude\s*=\s*\[([^\]]*)\]/m.exec(manifest)
  return m === null ? [] : [...m[1].matchAll(/"([^"]*)"/g)].map((x) => x[1])
}

const files = {}
for (const namespace of dirs(root))
  for (const name of dirs(join(root, namespace)))
    for (const version of dirs(join(root, namespace, name))) {
      const home = join(root, namespace, name, version)
      const manifest = readFileSync(join(home, "typst.toml"), "utf8")
      const skip = excluded(manifest)
      for (const path of walk(home)) {
        const rel = relative(home, path).split(sep).join("/")
        const kept = rel === "typst.toml" || rel.endsWith(".typ")
        if (kept && !skip.some((x) => rel === x || rel.startsWith(`${x}/`))) {
          files[`@${namespace}/${name}:${version}/${rel}`] = readFileSync(path, "utf8")
        }
      }
    }

if (Object.keys(files).length === 0) {
  console.error(`typst-packages.mjs: no package under ${root}`)
  process.exit(1)
}
const sorted = Object.fromEntries(Object.entries(files).sort(([a], [b]) => (a < b ? -1 : 1)))
process.stdout.write(`${JSON.stringify(sorted)}\n`)
