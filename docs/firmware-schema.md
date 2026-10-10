# Firmware documentation schema

This document describes the JSON Schema that a firmware release uses to describe itself.
The schema is in [`schemas/firmware/v2/firmware.schema.json`](../schemas/firmware/v2/firmware.schema.json).
A full example is in [`schemas/firmware/v2/examples/autococker-26.29.0.json`](../schemas/firmware/v2/examples/autococker-26.29.0.json).

## Goals

The firmware document has three jobs.

1. **Self-description.** The firmware states its menus, how the menus work, and what each
   setting does. The manual needs no other source for the on-device user interface.
2. **An explicit menu tree.** The document holds the screens and the rows that open them.
   The generator builds a tree from them and draws a navigation graphic. A reader selects a
   screen in the graphic and jumps to its section of the manual.
3. **Shared keys.** Each setting and each topic has a key that the application documentation
   also uses. The generator shows the firmware text and the application text for one key
   side by side, with one tab for the board and one tab for the configuration app, like the
   platform tabs in the Kotlin reference documentation.

## Where the document lives

The firmware repository writes the document at each release and attaches it to that
release as the asset `<board>-<version>-manual-document.json`. The manifest of a manual
names the release and the asset, and the generator downloads the document, verifies it
against the digest of the release, and validates it before it renders anything. Run
`snapshotpb-manual validate firmware <file>` to check a document alone.

## Where the data comes from

The firmware holds its menu tree in `app/menu/menu_tree.yaml`, and `scripts/menu_gen.py`
compiles that file into the firmware tables. The script `scripts/manual_firmware_document.py`
of the firmware repository reads the same sources and writes the firmware document, so the
document stays true to the firmware that ships. The prose that the sources do not hold
comes from `docs/manual/firmware-words.toml` of that repository, which a person keeps. The
table at the end of this file lists what that words file holds.

## Versioning

The document carries two versions.

| Field | Meaning |
|---|---|
| `schema_version` | The version of the schema that the document follows. The major number is the folder name under `schemas/firmware/`. |
| `firmware.version` | The firmware release that the document describes. This value versions the document. |

A document is stored as `firmware/<board>/<firmware.version>.json`. The generator builds a
manual for one firmware version and one application version. The field
`firmware.protocol_version` lets the generator pair a firmware document with an application
document that speaks the same protocol.

## Keys

The schema has two kinds of key.

- **A local key** names a thing inside one document: a control, an interaction, a screen,
  or a predicate. It is lower case with digits and underscores, for example `fire_menu`.
- **A shared key** names a thing that the firmware and the application both document. It has
  two parts with a dot between them, for example `autococker.eye_mode`.

A shared key for a setting is the field path in the protocol model: `<scope>.<field>`. The
scopes are the protocol messages `device_config`, `profile`, and `autococker`, and the word
`runtime` for a value that the board does not store. The firmware menu tree already binds each
row with this path, and the application maps the same protocol fields, so neither side needs a
new registry. A shared key for a topic that is not a setting uses the scope `topic`, for example
`topic.lockout`.

## Document structure

The top level of the document has these members.

| Member | Content |
|---|---|
| `firmware` | The name, the version, the protocol version, the board models, and a summary. |
| `controls` | The physical buttons. Each control lists the events it sends: a short press, a long press with its hold time. |
| `interactions` | What a user can do, and which control and event does it. The five required entries are `open_menu`, `cursor_up`, `cursor_down`, `select`, and `back`. Each entry says where it applies: the home screen, a menu screen, an editor, or the locked state. |
| `home` | The screen that shows when no menu is open, and the meaning of each mark on it. |
| `root` | The key of the screen that `open_menu` opens. |
| `screens` | The menu screens. Each screen has a title and an ordered list of rows. |
| `settings` | The settings, keyed by shared key. Each setting has a title, a description, a scope, a type, and the constraints of that type. |
| `predicates` | The conditions that hide or disable a row, with what each one means and which settings decide it. |
| `topics` | Documentation that is not a setting: the power-off gesture, the lockout, the eye override. |
| `media` | The media items that the document shows: pictures, 3D models, videos, and kinds that a later version defines. See the section on media. |

### Rows

A row on a screen has one of three types.

- A **setting** row names a setting by its shared key. The setting entry holds the description
  and the constraints. The row holds only the text that the panel draws.
- A **submenu** row names the screen that it opens by its local key.
- A **back** row returns to the parent screen, or from the root screen to the home screen.

A row can carry `hidden_when` or `disabled_when` with the key of a predicate. A back row
cannot.

### Settings

A setting has one of three types.

| Type | Constraints |
|---|---|
| `int` | `min`, `max`, `step`, an optional `unit`, `decimals`, `zero_text`, and `default`. The stored value is the drawn value times ten to the power of `decimals`. |
| `enum` | A list of `options`. Each option has the protocol symbol or number it stores, the text the panel draws, and a description of what the option does. |
| `toggle` | An optional `default`, and the text for each state. |

Each setting has a `scope`: `device` for one value on the board, `profile` for one value in each
profile, and `runtime` for a value that the board resets at each boot. A setting can list
`related` shared keys. The manual links them.

## Media

A document can show a file beside its text: a picture of the buttons, a 3D model of the board,
or a video of a procedure. The `media` map of the document holds each item under a local key.
A setting, a topic, a screen, a control, the home screen, or the firmware itself names the items
it shows in its `media` list, in the order the manual shows them.

| Field | Meaning |
|---|---|
| `kind` | What the file is. The known kinds are `image`, `model`, `video`, and `document`. The field is open: a kind that the generator does not know is still valid, and the generator embeds the file with a link and the caption. |
| `src` | Where the file is. A plain path is relative to the document file. A path with a `<source>:` prefix, for example `pcb:renders/board.glb`, is relative to that source root of the manual. The manifest of the manual names the source roots, so the board repository can provide a model that the firmware document shows. |
| `caption` | The caption under the item, and its accessible name. |
| `media_type` | The IANA media type. The generator infers it from the file extension when it is absent. |
| `alt` | A text alternative, when the caption is not enough. |
| `params` | Parameters for the kind, such as a camera position for a model. The schema does not constrain them. The generator writes them into the figure as a data attribute, so a viewer for the kind can read them from the page. |

The generator shows an `image` inline, a `video` with the controls of the browser, and each
other kind as an embedded file with a link. An interactive viewer for a `model` is a later step.
The manual is one file with no link to the internet, so a viewer must be small enough to travel
with the page.

The common definitions, the media item and the key patterns, are in
[`schemas/common/v2/media.schema.json`](../schemas/common/v2/media.schema.json). The firmware
schema and the application schema both refer to it.

## The menu tree

The tree is explicit. The generator starts at `root` and follows each submenu row. The schema
cannot state every rule of a tree, so the generator checks these rules after schema validation.

- The `root` key names a screen in `screens`.
- Each screen except the root is the target of exactly one submenu row.
- The root is the target of no submenu row.
- Each screen is reachable from the root.
- The last row of each screen is a back row.
- Each setting row names a key in `settings`.
- Each predicate that a row names is in `predicates`.
- Each `related` key names a setting or a topic.
- Each interaction names a control and an event that the control sends.

From the tree the generator draws the navigation graphic. Each box is a screen, with its title
and its rows. Each solid edge is a submenu row, labelled with the row text. Each dashed edge is
the back row. Each box links to the section of the manual for that screen. The firmware
repository already draws this map with Mermaid in `docs/menu/menu-map.md`, and the manual
draws the same shape from the document.

## Localized text

Each text that the manual prints is a `localized` value of the common schema: an array with
one item for each language, `{"lang": "en", "text": "Fire mode"}`. The language is a tag
such as `en`, `de`, or `es-MX`. The English item is required, and each language appears
once. The `text` definition is prose for the manual, and the `label` definition is a short
text that the manual prints as it is: a name, a title, a row, an option, or a path. A `render`
holds 16 characters in each language, as the panel does.

The generator reads a document for one language, which the manifest or `--lang` names. It
takes the item of that language. When there is none, it takes the first item of the same
language with another region, in document order, and then the English item. Put the item
with no region first, so that `de` finds `de` before `de-AT`. The typed model holds the
words of one language, so the rest of the generator does not know about languages.

The generator finds a localized text by its shape: an array whose items are objects with
the two members `lang` and `text` alone. No other value of the schema set has that shape,
and a later version of the schema set must not add one. The `params` member of a media
item is free-form and is not read for text.

A key, a protocol symbol, a version, a unit, and a path to a file are not localized. They
stay plain strings.

## Tabs between firmware and application

The application document uses the same shared keys. For each key the generator collects the
firmware entry and the application entry. It renders one section for the key with two tabs:
the board, and the configuration app. The app tab shows the words once when each platform
gets the same words, with the path of each platform when only the path differs. When the
words differ, the app tab holds one tab for each platform, in the order of the application
document. A platform that has no entry for the key has no tab.
A platform can state that it shows the setting but does not let the user edit it. The tabs
work with no script. With the script, a choice on one section selects the same tab on every
section, and the page remembers it. The application schema is in
[`docs/app-schema.md`](app-schema.md).

## What the words file holds

The firmware menu tree holds the structure of the menu but not the prose. These fields come
from `docs/manual/firmware-words.toml` of the firmware repository. The column Today says
where the firmware holds the information, when it holds it at all.

| Field | Where | Today |
|---|---|---|
| `description` for each setting | `settings.<key>.description` | Not in the menu tree. The design notes in the YAML comments hold some of the text. |
| `description` for each enum option | `settings.<key>.options[].description` | Not in the menu tree. |
| `default` for each setting | `settings.<key>.default` | In the driver, not in the menu tree. |
| `description` for each predicate | `predicates.<key>.description` | In the YAML comments. |
| `hold_ms` for each long press | `controls.<key>.events.long.hold_ms` | In the input adapter. |
| The topics | `topics` | In `docs/menu-state-machine-design.md` and `docs/lockout-semantics.md`. |

## Open points

- **Languages.** The firmware draws four languages, and version 2.0.0 of the schema holds
  each text in each language. The chapters of a manual and the words of the page itself are
  in English. A manual in another language needs chapters in that language too.
- **The `render` limit.** The schema limits a row text to 16 characters, which is the limit of
  the panel today. A firmware with a larger panel needs a larger limit in a later schema version.
- **The editor states.** The schema names five places where an interaction applies: `home`,
  `screen`, `edit_int`, `edit_enum`, and `locked`. A new editor type needs a new value.
