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

- **`common/v1`** holds the definitions that the other two share: the key patterns, and the
  media item that lets a document show a picture, a 3D model, a video, or a later kind.
- **`firmware/v1`** describes a firmware revision: its controls, how its menu works, its
  screens, its settings, and its topics. The screens form an explicit tree, and the
  generator draws the tree as a graphic that links to each section.
- **`app/v1`** describes an application revision: the platforms it runs on, and what it
  shows for each setting and topic of the firmware, by the same key.

A setting has one shared key in both documents, which is its field path in the protocol,
for example `autococker.eye_mode`. The manual shows the firmware words and the app words
for one key side by side, with one tab for the board and one tab for each platform.

## Versioning

Each document carries two versions. The `schema_version` names the schema that the document
follows. The `firmware.version` or the `app.version` is the release that the document
describes, and it versions the document. A document is stored by that version:
`firmware/<board>/<version>.json` and `app/<name>/<version>.json`. A manual is built for one
firmware version and one app version, which its manifest names.

## Build a manual

```sh
cargo build --release
./target/release/snapshotpb-manual build manifests/snapshot-v2.toml
```

The build writes `dist/snapshot-v2.html`. `--out <file>` writes to a different file, and
`--source <name>=<path>` replaces a source root of the manifest. The other commands:

```sh
./target/release/snapshotpb-manual check manifests/snapshot-v2.toml
./target/release/snapshotpb-manual validate firmware firmware/snapshot_v2/26.29.0.json
./target/release/snapshotpb-manual validate app app/snapshotpb/26.40.1.json
```

`check` renders the manual and writes nothing. `validate` checks a document against its
schema and against the rules that the schema cannot state, such as the shape of the menu
tree. A fault stops the command with the path of the fault in the document.

## The layout of the repository

| Path | What it holds |
|---|---|
| `schemas/` | The schema set, with an example firmware document under `firmware/v1/examples/` |
| `firmware/<board>/<version>.json` | One firmware document for each firmware release |
| `app/<name>/<version>.json` | One application document for each app release |
| `manifests/<id>.toml` | One manifest for each manual: the documents, the sources, and the brand |
| `content/<id>/*.md` | The chapters of a manual, in file order. A chapter is Markdown with directives. |
| `assets/` | The logo, the icon, the stylesheet and the script. The generator embeds them. |
| `src/` | The generator, in Rust |
| `docs/` | The design of the schema set |

## Write a chapter

A chapter is a Markdown file. A `#` heading is a chapter of the page, and a `##` heading is
a section. A paragraph that starts with `WARNING:`, `CAUTION:` or `NOTE:` becomes a marked
block. A line that holds one directive puts a generated block there:

| Directive | Block |
|---|---|
| `{{menu}}` | The settings reference: each screen in the order of the tree, its rows, and one section for each setting with its tabs |
| `{{menu-map}}` | The menu tree as a graphic with a link on each screen, and as a list |
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
`app.version`, `app.summary`, `board.md5`, `board.name`, `manual.date`,
`control.<key>.name`, `control.<key>.hold_ms`, or a key of the `[values]` table of the
manifest.

A path in a manifest, in a directive, or in the `src` of a media item is `name:rest` for a
file under the source root `name`, `~/…` for a file under the home directory, or a path
relative to the manifest or to the document.

## Add a firmware release or an app release

1. Write the firmware document as `firmware/<board>/<version>.json`. The firmware repository
   holds the menu tree, so its generator is the right place to write the document from.
2. Write the app document as `app/<name>/<version>.json`, with the same shared keys.
3. Run `validate` on each document.
4. Copy `manifests/snapshot-v2.toml` to `manifests/<id>.toml`, name the documents, and
   change the sources and the board.
5. Copy `content/snapshot-v2/` to `content/<id>/`, and change the chapters.
6. Build, and read the manual in a browser in both colour schemes.

## Documentation

- [`docs/firmware-schema.md`](docs/firmware-schema.md) describes the firmware schema:
  the controls, the screens, the settings, the menu tree, the media, and the shared keys.
- [`docs/app-schema.md`](docs/app-schema.md) describes the application schema: the
  targets, the entries, and the overrides for one target.

## License

The source code is licensed under the GNU General Public License, version 3. See
[`LICENSE`](LICENSE). The Snapshot PB name, logos, and icons are trademarks and are not
covered by that license. See [`TRADEMARKS.md`](TRADEMARKS.md).
