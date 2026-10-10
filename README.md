# SnapshotPB Manual Generator

`snapshotpb-manual` writes the manual of a SnapshotPB board and its firmware. The manual
is one HTML file. Every style, script and picture is inside the file, so the manual works
from any folder, with no connection, and after a download. The page follows the light and
the dark colour scheme of the SnapshotPB app, and a button on the page changes the scheme.

The tables of the manual come from the sources, and not from a person:

| Source | What it gives the manual |
|---|---|
| `app/menu/menu_tree.yaml` of the firmware | Each screen and each row of the menu: its type, its range, its unit, its options and its conditions |
| `app/menu/menu_lang.yaml` of the firmware | The words of the panel in each language, and the captions |
| `docs/menu/interaction-fsm.md` of the firmware | What each button does on each screen |
| The protocol `.proto` files | The meaning of each setting and each option, where the author gave no words |
| `Kconfig` files and C headers of the firmware | The factory defaults and the fixed times |
| `METRICS.json` of the board | The board figures and the board `md5` |
| `Color.kt` of the app | The two colour schemes |

A change to one of these sources changes the manual at the next build. A row that binds a
field that the protocol lost, or a state of the FSM that the manifest does not name, stops
the build with a message.

## Build

```sh
cargo build --release
./target/release/snapshotpb-manual build manifests/snapshot-v2.toml
```

The build writes `dist/snapshot-v2.html`. `--out <file>` writes to a different file, and
`--source <name>=<path>` replaces a source root of the manifest:

```sh
./target/release/snapshotpb-manual build manifests/snapshot-v2.toml \
    --source firmware=~/src/Firmware \
    --out ~/workspace/SnapshotPB-PCB/snapshot_v2/renders/manual/snapshot-v2.html
```

`check` reads every source and renders the manual, and it writes nothing:

```sh
./target/release/snapshotpb-manual check manifests/snapshot-v2.toml
```

## The layout of the repository

| Path | What it holds |
|---|---|
| `src/` | The generator, in Rust |
| `assets/` | The logo, the icon, the stylesheet and the script. The generator embeds them. |
| `manifests/<id>.toml` | One manifest for each manual: the sources, the words for the states and the guards, and the source of each default |
| `content/<id>/*.md` | The chapters, in file order. A chapter is Markdown with directives. |
| `content/<id>/settings.toml` | The words of the author for each setting, each option and each caption |

## Write a chapter

A chapter is a Markdown file. A `#` heading is a chapter of the page, and a `##` heading is
a section. A paragraph that starts with `WARNING:`, `CAUTION:` or `NOTE:` becomes a marked
block. A line that holds one directive puts a generated block there:

| Directive | Block |
|---|---|
| `{{menu}}` | The settings reference: one table for each screen |
| `{{menu-map}}` | The tree of the screens |
| `{{buttons}}` | One table for each screen of the interaction FSM |
| `{{enum Name}}` | The values of a protocol enum, with their meaning |
| `{{languages}}` | Each word of the panel in each language |
| `{{captions}}` | The captions that `settings.toml` describes |
| `{{board}}` | The board figures |
| `{{provenance}}` | Each source with its version |
| `{{image <path> \| <caption>}}` | A picture. An SVG goes inline, and a PNG or a JPEG goes in as data. |

`{{value <name>}}` in a line prints a value: `firmware.version`, `board.md5`,
`board.name`, `manual.date`, `define.<NAME>`, `kconfig.<NAME>`, or a key of the `[values]`
table of the manifest.

A path in a manifest or in a directive is `name:rest` for a file under the source root
`name`, `~/…` for a file under the home directory, or a path relative to the manifest.

## Add a board or a firmware

1. Copy `manifests/snapshot-v2.toml` to `manifests/<id>.toml`, and change the sources, the
   board and the firmware target.
2. Map each `bind` scope of the menu tree to its protocol message in `[firmware.scopes]`.
3. Give words to each event, each guard and each state of the interaction FSM. The build
   stops when the FSM has a state with no words.
4. Name the source of each factory default in `[defaults]`.
5. Copy `content/snapshot-v2/` to `content/<id>/`, and change the chapters.
6. Build, and read the manual.

## Check the manual

The build stops on an unknown directive, on a link to an id that the page does not have,
on a `bind` with no protocol field, and on a default that the firmware does not define.
Read the manual in a browser in both colour schemes before you publish it.
