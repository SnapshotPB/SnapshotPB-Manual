# SnapshotPB Manual Generator

A repeatable generator that builds one consolidated manual from the firmware
documentation and the application documentation of Snapshot PB.

## What it does

The generator reads a set of structured documentation files. It validates each file
against a defined schema set. It then renders the files into one manual that covers
the firmware and the applications together. Each run with the same inputs gives the
same manual.

## Schema set

The documentation files follow [JSON Schema](https://json-schema.org/). The schema set
has two parts:

- **Firmware schemas** describe a firmware revision: the board, its settings, its
  modes, and its behavior.
- **Application schemas** describe an application revision: its screens, its
  settings, and the firmware features that it exposes.

## Versioning

Each schema carries a version. The version of a firmware schema is the firmware
version that it describes. The version of an application schema is the application
version that it describes. A manual is built for one firmware version and one
application version, so the generator selects the matching schema for each.

## License

The source code is licensed under the GNU General Public License, version 3. See
[`LICENSE`](LICENSE). The Snapshot PB name, logos, and icons are trademarks and are not
covered by that license. See [`TRADEMARKS.md`](TRADEMARKS.md).
