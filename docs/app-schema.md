# Application documentation schema

This document describes the JSON Schema that an application release uses to describe
itself on each runtime. The schema is in
[`schemas/app/v2/app.schema.json`](../schemas/app/v2/app.schema.json). The document for the
current app is in [`app/snapshotpb/`](../app/snapshotpb/).

## Goal

The app runs on six runtimes: Android, iOS, Desktop, Browser, Wear OS, and Zepp OS. They do
not all work the same way. The manual has one page for every runtime. The reader selects the
runtimes that matter once, and each section then shows only the words for those runtimes:
side by side where they differ, and merged where they agree. This is the concept of a
platform reference that covers many targets on one page.

## What the survey of the app found

The runtimes fall into two user interfaces.

- **The shared user interface** runs on Android, iOS, Desktop, and Browser. It is one
  Compose application. On these four, each setting is shown and edited the same way, and
  the screens are the same. The differences are at the edges: how Bluetooth gets its
  permission, how the reader goes back, how a photo is taken, and the browser that is
  supported.
- **The watch user interfaces** run on Wear OS and Zepp OS. Each is a hand-written app with
  fewer screens. Both edit the active profile only, use steppers for numbers, offer a preset
  list for the screen-off delay, and have no boot text, no display window, no profile naming,
  and no timing diagram. They differ from each other in sync with the phone, in languages,
  in how lockout shows, and in the save messages.

So a variant keyed by a runtime alone would repeat the same words four times for the shared
interface and twice for the watches. The schema therefore has **groups**.

## Runtimes and groups

| Member | Content |
|---|---|
| `runtimes` | The platforms, keyed by runtime key, in the order of the runtime selector. Each has a name, an optional group, a description, and an optional `since` version. The key `board` is reserved for the board itself. |
| `groups` | The sets of runtimes that share one user interface, keyed by group key: `shared` and `watch` for this app. |

A **selector** is a runtime key or a group key. Each place in the document that differs by
runtime takes a selector.

## Entries and variants

A setting or a topic is an **entry**. The fields at the top of the entry hold on every
runtime: the title, the description, the path in the app, whether the app can edit the value,
the steps of a procedure, the media, and the `since` version.

The `variants` map changes those fields for one selector. A runtime variant wins over a group
variant, and a group variant wins over the top of the entry. A variant with `absent: true`
removes the entry for that selector. An entry with no `variants` applies to every runtime.

The generator resolves the entry for each runtime, then merges the runtimes whose resolved
words are the same into one block with one badge for each runtime. For a setting that the
shared interface and the watches show differently, the reader sees two blocks: one with four
badges and one with two.

## Settings in the app only

A setting with the key of a firmware setting joins the firmware section of that setting, as a
block beside the board block. A setting that the firmware document does not have, such as the
boot text, the display window, or the device name, is marked `app_only: true`. The manual
renders it in the app chapter. The generator refuses a key that is in neither place and not
marked `app_only`, so a mistyped key cannot vanish without an error.

## Screens

The app has screens as the firmware has menu screens. Each screen names the runtimes or the
groups that have it, so the shared interface and each watch interface have their own screen
set. `roots` names the first screen of each group. The screens of a group form a tree, and the
generator draws one tree for each group and links each box to its section.

A screen lists its items in order: a setting by shared key, a screen that it opens with the
control that opens it, an action with what it does, and information that it shows. An item
can name the runtimes that have it, when not every runtime of the screen does. A screen can
carry variants for its description or its items.

## Features

A feature is a capability that differs between runtimes: add a profile, take a photo, sync
with the phone. The `features` map, keyed by `feature.<name>`, gives the level of support for
each selector: `yes`, `limited` with a note, or `no`. The generator renders a matrix of feature
by runtime, filtered by the runtime selector.

## Topics and procedures

A topic is an entry too. A topic with the key of a firmware topic joins it in the manual, as a
block beside the board block. A topic that only the app has stands alone in the app chapter.
A procedure gives its `steps` as a list of instructions, and a variant can replace the steps
for one selector.

## The runtime selector in the manual

The manual has a row of chips under the header: the board, then each runtime in the order of
the document, grouped. The reader selects any set of runtimes. Each block of the page carries
the runtimes it applies to. A block shows while one of its runtimes is selected. A section
whose every block is hidden folds to one line that says so. The page remembers the selection.
With no script, every block shows, each with its badges, so the manual reads in print and in a
reader with no script.

## Versioning

| Field | Meaning |
|---|---|
| `schema_version` | The version of the schema that the document follows. Version 2 replaces the `targets` map of version 1 with `runtimes`, `groups`, and `variants`, and adds `screens`, `features`, `steps`, and `since`. |
| `app.version` | The app release that the document describes. This value versions the document. |
| `app.protocol_version` | The protocol the app speaks. The generator refuses a pair with the firmware document that differs. |
| `since` | On a runtime, an entry, a variant, or a support level: the app version that first had it. The manual shows it as a badge. |

A document is stored as `app/<name>/<app.version>.json`.

## Rules that the generator checks

- Each selector in a variant, a screen, an item, a feature, or `roots` is a runtime key or a
  group key of the document.
- Each runtime that names a group names one in `groups`, each group has at least one
  runtime, and no key is both a group key and a runtime key.
- No runtime uses the key `board`.
- Each setting is a firmware setting or is marked `app_only`.
- Each media key that an entry, a variant, a screen, or a runtime names is in `media`.
- Each screen that an item opens exists, and each setting that an item names is in `settings`.
- Each group in `roots` names a screen that the group has, and each screen of a group is
  reachable from that root through the items of any runtime of the group. A screen that
  two items open is placed once, under the first parent that the walk met.
- Each feature covers every runtime through a runtime key or a group key.
- The protocol version of the app matches the protocol version of the firmware document when
  both state one.

## Where the document comes from

The app repository holds the screen keys, the field table, the strings, and the per-runtime
seams that the document describes. The document in this repository is hand-written from a
survey of that code. The next step is a generator in the app repository that writes the
document from the field table and the strings, with the runtime differences stated once in a
capability table there, so the document stays true to the app that ships.
