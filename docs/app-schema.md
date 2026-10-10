# Application documentation schema

This document describes the JSON Schema that an application release uses to describe
itself. The schema is in [`schemas/app/v1/app.schema.json`](../schemas/app/v1/app.schema.json).
The document for the current app is in [`app/snapshotpb/`](../app/snapshotpb/).

## Goal

The application document gives the words of the app for each shared key of the firmware
document. The generator shows the firmware words and the app words for one key side by side,
with one tab for the board and one tab for the configuration app. On most keys, each
platform gets the same words, and the app tab shows them once. Where the words differ, the
app tab holds one tab for each platform. A reader selects a platform once, and each such
key shows that platform.

## Versioning

| Field | Meaning |
|---|---|
| `schema_version` | The version of the schema that the document follows. |
| `app.version` | The app release that the document describes. This value versions the document. |
| `app.protocol_version` | The protocol the app speaks. When the firmware document states a protocol version too, the generator refuses a pair that differs. |

A document is stored as `app/<name>/<app.version>.json`.

## Document structure

| Member | Content |
|---|---|
| `app` | The name, the version, the protocol version, a summary, and media. |
| `targets` | The platforms, keyed by target key, in the order of the platform tabs. Each target has a name and an optional description. The keys `board` and `app` are reserved for the tabs of the manual itself. |
| `settings` | One entry for each setting the app shows, keyed by the shared key of the firmware document. |
| `topics` | One entry for each topic, keyed by the shared key. A topic that only the app has is permitted. |
| `media` | The media items of the document. See [`docs/firmware-schema.md`](firmware-schema.md). |

### Entries

An entry has a title, a description, an optional `where`, which is the path to the setting in
the app, an optional `editable`, and optional media. The fields at the top of the entry apply
to every target.

The `targets` map of an entry does two things. It limits the entry to the targets it names. It
overrides the description, the path, the editable flag, or the media for one target. An empty
override, `{}`, keeps the words at the top of the entry for that target. An entry with no
`targets` map applies to every target of the document.

The generator resolves the entry for each target. When each target gets the same words, the
app tab shows them once. When the entry does not cover each target of the document, the app
tab names the platforms that have it. When an override changes the words for one target, the
app tab holds one tab for each target of the entry.

## Rules that the generator checks

- Each target that an entry names is in `targets`.
- No target has the key `board` or the key `app`.
- Each media key that an entry names is in `media`.
- The protocol version of the app matches the protocol version of the firmware document when
  both state one.
