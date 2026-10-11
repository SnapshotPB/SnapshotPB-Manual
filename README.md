# SnapshotPB Manual Generator

A repeatable generator that builds one consolidated manual from the firmware
documentation and the application documentation of Snapshot PB.

## What it does

The generator reads a firmware document and an application document. It validates each
document against a defined schema set. It then renders the documents into one HTML manual
that covers the board, the firmware, and the app together. Each run with the same inputs
gives the same manual.

The manual is one file. Every style, script and picture is inside the file, so the manual
works from any folder, with no connection, and after a download. The page follows the light
and the dark colour scheme of the SnapshotPB app, and a button on the page changes the scheme.

## Schema set

The documents follow [JSON Schema](https://json-schema.org/), draft 2020-12. The schema set
has three parts under `schemas/`:

- **`common/v2`** holds the definitions that the other two share: the localized text, the
  key patterns, and the media item that lets a document show a picture, a 3D model, a
  video, or a later kind.
- **`firmware/v2`** describes a firmware revision: its controls, how its menu works, its
  screens, its settings, and its topics. The screens form an explicit tree, and the
  generator draws the tree as a graphic that links to each section.
- **`app/v2`** describes an application revision: the platforms it runs on, and what it
  shows for each setting and topic of the firmware, by the same key.

A setting has one shared key in both documents, which is its field path in the protocol,
for example `autococker.eye_mode`. The manual shows the firmware words and the app words
for one key side by side, with one tab for the board and one tab for the configuration app.
The app tab shows the words once when each platform gets the same words, with the path of
each platform when only the path differs. It shows one tab for each platform when the words
differ.

Each text that the manual prints is localized. A document holds it as an array with one
item for each language, for example `[{"lang": "en", "text": "Fire mode"}, {"lang": "de",
"text": "Feuermodus"}]`. The English item is required. The manifest names the language of
the manual, and `--lang` replaces it. A text that has no item in that language falls back
to the same language with another region, and then to English. The chapters and the words
of the page itself are in English.

Version 1 of the schema set, under the `v1` folders, held each text as one English string.
The generator reads version 2 alone.

## Versioning

Each document carries two versions. The `schema_version` names the schema that the document
follows. The `firmware.version` or the `app.version` is the release that the document
describes, and it versions the document. A manual is built for one firmware release and
one app release, which its manifest names.

## Where the documents come from

The firmware repository and the app repository each hold a generator that writes their
document from their sources, and their release workflows attach the document to each
release. This repository stores no document. The manifest names the release and the asset:

```toml
[firmware]
release = "SnapshotPB/Firmware@v2026.41.1"
asset = "snapshot_reva-2026.41.1-manual-document.json"
sha256 = "579d44d6…"   # optional: the build refuses an asset with another digest
```

The build reads the release through the `gh` command, downloads the asset into the cache
directory, and verifies its SHA-256 against the digest that GitHub holds for the asset,
and against `sha256` when the manifest pins one. A cached asset that verifies is used
with no download. The repositories are private, so `gh` must be signed in, or `GH_TOKEN`
must hold a token with read access to their contents. The cache is
`$SNAPSHOTPB_MANUAL_CACHE`, or `~/.cache/snapshotpb-manual`.

A `document = "<path>"` member in the place of `release` and `asset` names a file, for a
document that is not released yet.

## Build a manual

```sh
cargo build --release
./target/release/snapshotpb-manual build manifests/autococker-rev-b.toml
```

The build writes `dist/snapshotpb-autococker-rev-b.html`. `--out <file>` writes to a different file, and
`--source <name>=<path>` replaces a source root of the manifest. The path is relative to the
current directory, not to the manifest.

The `site` command writes the site of GitHub Pages: one manual for each manifest and each
language that its documents hold, and an index page where the reader picks a board and a
language.

```sh
./target/release/snapshotpb-manual site manifests/autococker-rev-b.toml --out site
```

It writes `site/<id>/<lang>/index.html` for each language, where `<id>` is the id of the
manifest. The page `site/<id>/index.html` opens the language of the manifest, and
`site/index.html` is the index. The languages are each tag that a localized text of the
firmware document or the app document names, and the language of the manifest. The chapters
under `content/` are English in each language until a chapter set exists for that language.

The other commands:

```sh
./target/release/snapshotpb-manual check manifests/autococker-rev-b.toml
./target/release/snapshotpb-manual validate firmware ~/.cache/snapshotpb-manual/SnapshotPB/Firmware/v2026.41.1/snapshot_reva-2026.41.1-manual-document.json
./target/release/snapshotpb-manual validate app ~/Downloads/snapshotpb-app-26.41.2-manual-document.json
```

`check` renders the manual and writes nothing. `validate` checks a document against its
schema and against the rules that the schema cannot state, such as the shape of the menu
tree. A fault stops the command with the path of the fault in the document.

## Release a manual

A release is a GitHub Release with the manual as its asset. The version is calendar-derived,
and the git tag is the single source of truth. The scheme is the same as the SnapshotPB app:
`<2-digit ISO week-year>.<ISO week>.<count this week>`, for example `26.41.1`. The tag adds
a `v`: `v26.41.1`.

Two workflows make a release:

1. **Release (cut tag)** (`.github/workflows/release.yml`). An operator runs it from the
   Actions tab. It computes the next version from the tags that exist, pushes the annotated
   tag, and dispatches the build workflow against the tag. It refuses a commit that is not
   on `main`.
2. **Build Release** (`.github/workflows/build-release.yml`). It gets the version from the
   tag, builds the generator, builds the manual, and publishes the manual as an asset of a
   GitHub Release. The name of the Release is the version, and its tag is the tag. The asset
   is always `snapshotpb-autococker-rev-b.html`, so the link
   `releases/latest/download/snapshotpb-autococker-rev-b.html` gives the latest manual.

The manifest names two source repositories: the app, for the theme, and the PCB, for the
board metrics and the pictures. The build workflow clones the default branch of each one
with the `SOURCES_TOKEN` repository secret. The same secret lets `gh` download the two
documents from their releases. The secret is a fine-grained personal access token with read
access to the contents of `SnapshotPB/Configuration-App`, `SnapshotPB/PCB-Autococker`, and
`SnapshotPB/Firmware`. The build stops with a clear error when the secret is not set.

Two more workflows run on their own:

- **Check** (`.github/workflows/check.yml`). It runs on a pull request that touches the code
  of the generator: `src/`, `schemas/`, `assets/`, or the Cargo manifest. It checks the
  format, runs clippy, and runs the tests. A pull request that changes the content alone
  does not start it.
- **Publish to GitHub Pages** (`.github/workflows/pages.yml`). Each push to `main` that
  changes the manual builds the site with the `site` command and publishes it as the Pages
  site. The workflow needs the `SOURCES_TOKEN` secret, and the Pages source of the
  repository must be "GitHub Actions".

Each workflow caches the Cargo registry and the compiled dependencies, keyed on
`Cargo.lock` and the toolchain. The Pages workflow also caches the two downloaded documents,
keyed on the manifest.

`scripts/cut_version.py` is the compute core of the first workflow. The same scheme in two
repositories keeps the numbers of the app and of the manual in step.

```sh
python3 scripts/cut_version.py --dry-run           # show the next version and tag
python3 -m unittest discover -s scripts/tests      # test the compute core
```

## The layout of the repository

| Path | What it holds |
|---|---|
| `schemas/` | The schema set, with an example firmware document under `firmware/v2/examples/` |
| `manifests/<id>.toml` | One manifest for each manual: the releases of the documents, the sources, and the brand |
| `content/<id>/*.md` | The chapters of a manual, in file order. A chapter is Markdown with directives. |
| `assets/` | The logo, the icon, the stylesheet and the script. The generator embeds them. |
| `src/` | The generator, in Rust |
| `docs/` | The design of the schema set |
| `scripts/` | The version script of the release workflow, with its tests |
| `.github/workflows/` | The two release workflows |

## Write a chapter

A chapter is a Markdown file. A `#` heading is a chapter of the page, and a `##` heading is
a section. A paragraph that starts with `WARNING:`, `CAUTION:` or `NOTE:` becomes a marked
block. A line that holds one directive puts a generated block there:

| Directive | Block |
|---|---|
| `{{menu}}` | The settings reference: each screen in the order of the tree, its rows, and one section for each setting with its tabs |
| `{{menu-map}}` | The menu tree, as a graphic on a wide screen and as a list on a narrow screen |
| `{{controls}}` | The buttons of the board and what each press does |
| `{{interactions}}` | What the user can do, where, and with which button |
| `{{home}}` | The home screen and its marks |
| `{{topic <key>}}` | One topic of the firmware document, with its tabs |
| `{{topics}}` | Every topic of the firmware document |
| `{{targets}}` | The platforms of the app |
| `{{media <key>}}` | One media item of the firmware document |
| `{{board}}` | The board figures |
| `{{provenance}}` | Each source with its version |
| `{{image <path> \| <caption>}}` | A picture from a source root. An SVG goes inline, and a PNG or a JPEG goes in as data. |

`{{value <name>}}` in a line prints a value: `firmware.version`, `firmware.summary`,
`app.version`, `app.summary`, `board.md5`, `board.name`, `manual.date`, `manual.language`,
`control.<key>.name`, `control.<key>.hold_ms`, or a key of the `[values]` table of the
manifest.

A path in a manifest, in a directive, or in the `src` of a media item is `name:rest` for a
file under the source root `name`, `~/…` for a file under the home directory, or a path
relative to the manifest or to the document.

The `[names]` table of the manifest gives a readable name to a designator that the board
labels in short form, for example `"TRIG" = "Trigger"`. A chapter or a document writes the
designator in backticks, and the page shows the name with the designator after it:
Trigger (`TRIG`). A label in an SVG drawing shows the name alone. A code block and a code
span with a class stay as they are. A designator starts and ends with a letter, a digit or
an underscore, and it can hold a dash or a dot between them.

## Add a firmware release or an app release

1. Cut the release in the firmware repository or in the app repository. Its release
   workflow writes the document and attaches it to the release.
2. In the manifest, name the new release and its asset, and pin the digest of the asset
   from the release page or from `gh api repos/<owner>/<repository>/releases/tags/<tag>`.
3. For a new manual, copy `manifests/autococker-rev-b.toml` to `manifests/<id>.toml` and
   change the sources and the board, and copy `content/autococker-rev-b/` to
   `content/<id>/` and change the chapters.
4. Build, and read the manual in a browser in both colour schemes.

## Documentation

- [`docs/firmware-schema.md`](docs/firmware-schema.md) describes the firmware schema:
  the controls, the screens, the settings, the menu tree, the media, and the shared keys.
- [`docs/app-schema.md`](docs/app-schema.md) describes the application schema: the
  targets, the entries, and the overrides for one target.

## License

The source code is licensed under the GNU General Public License, version 3. See
[`LICENSE`](LICENSE). The schema set under [`schemas/`](schemas/) is licensed under the
Mozilla Public License, version 2.0, so that another repository can use a schema file with
no change to its own license. See [`schemas/LICENSE`](schemas/LICENSE) and
[`schemas/README.md`](schemas/README.md). The Snapshot PB name, logos, and icons are
trademarks and are not covered by either license. See [`TRADEMARKS.md`](TRADEMARKS.md).
