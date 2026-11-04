# Solvercraft application and ArtCraft Launcher submission

This directory contains a **reviewable upstream catalog proposal**, an original
Solvercraft icon and release-contract checks. It does not register an app in the
official launcher, publish a release or assert ArtCraft endorsement.

The standalone `solvercraft` GUI application, Rust SDK and on-demand local stdio
MCP executable are delivered from `Puma7/gridcraft`. A launcher catalog entry is
a separate change in `storytold/craft-launcher`. The `storytold/artcraft` repository
is the image/video creation application, and its source is not the launcher
catalog. Gridcraft's `omastore.toml` does not dynamically register Solvercraft in
ArtCraft Launcher.

## Pinned upstream contract

The proposal targets ArtCraft Launcher commit
[`ec205366ebe42635e155b9ab246f036052fef406`](https://github.com/storytold/craft-launcher/tree/ec205366ebe42635e155b9ab246f036052fef406).
`submission.json` records the exact original Git blob IDs and changed-file IDs.
The text patch adds a community entry to `crates/core/src/catalog.rs`, a details
page, the Linux desktop category, the original icon mapping and attribution,
and contextual text for the ten existing translated catalogs. Translation text
is a clean-room draft from the English meaning; native-speaker review remains
appropriate. No existing download checks, owner constraints or signature rules
are weakened.

| Field                     | Proposal                                        |
|---------------------------|-------------------------------------------------|
| App ID / executable       | `solvercraft` / `solvercraft.exe`                |
| Display name              | Solvercraft                                     |
| Kind / publisher          | Community / Puma7                               |
| Source and release repo   | `Puma7/gridcraft`                                |
| Bundle ID                 | `io.github.puma7.solvercraft`                    |
| App release version / tag | `0.14.2` / `v0.14.2`                             |
| Windows portable asset    | `solvercraft-0.14.2-windows-x64-portable.zip`      |
| Linux contract only       | `solvercraft-0.14.2-linux-x86_64.tar.gz`           |
| Verified advertised OS    | Windows                                         |
| macOS installer           | Disabled; publisher Developer ID is unverified  |

The ZIP must contain `solvercraft.exe` at its root, or all files under one
wrapper directory with `solvercraft.exe` directly inside it. A bare
`solvercraft-app.exe`, a `windows-x86_64.zip` asset or an executable only under
`bin/` does not satisfy the upstream Windows portable contract. The tarball
contract uses `bin/solvercraft` (or `solvercraft` at archive root).

ArtCraft Launcher parses ordinary SemVer tags with an optional leading `v`.
`solvercraft-v0.1.0` is not a supported release tag. Because this proposal uses
the Gridcraft repository's release feed, subsequent stable releases must publish
both app assets together. A future separate Solvercraft repository is another
option, requiring a reviewed catalog-source change. Do not silently redirect
an already approved catalog source.

Release availability is deliberately **not asserted**. Publishing a branch or
this patch is not equivalent to publishing the versioned assets. A release needs
the correct platform filename and `SHA256SUMS.txt`; the upstream launcher verifies
downloads before installation. The details proposal contains no screenshot URLs,
so it cannot request images that have not been accepted by the shared assets
repository.

## Maintainer decision required for the official catalog

The exact upstream
[AGENTS.md rule, lines 74–79](https://github.com/storytold/craft-launcher/blob/ec205366ebe42635e155b9ab246f036052fef406/AGENTS.md#L74)
says:

> **Agents never land or approve a third-party app.**

It states that adding an unapproved non-`storytold/` repository, download source
or install path is the maintainer's decision, and that agents do not merge,
approve or push fixes to such an upstream PR. `Puma7/gridcraft` is outside the
official owner and is not the already approved SolidCraft community app.

Consequently this change remains an unapplied submission artifact in the
Solvercraft repository. The ArtCraft maintainers need to evaluate the publisher,
repository age/history, release signing and concrete packages before accepting
it. No upstream repository or PR is changed by the supplied validator. The user
can build and run Solvercraft directly while that independent review is pending.

## Checking the proposal

Python 3.11+ and Git are sufficient for the contract checker. It does not execute
the packaged application or claim a complete launcher build.

```sh
python validate_submission.py --self-test
python validate_submission.py --launcher /path/to/pinned/craft-launcher
```

`--launcher` checks the fourteen exact base-file identities and runs read-only
`git apply --check`. An optional owned scratch parent exercises patch application
on a new copy and checks every resulting Git blob and the icon:

```sh
python validate_submission.py --launcher /path/to/pinned/craft-launcher --scratch-parent /path/to/owned/work
```

The source checkout itself remains unchanged. Scratch copies are retained for
inspection. Check an actual locally built Windows archive and its sums file with:

```sh
python validate_submission.py --package /path/to/solvercraft-0.14.2-windows-x64-portable.zip
```

The archive check verifies the platform name, exact checksum, bounded safe ZIP
layout, executable path and PE x64 header. It is an installation-contract check,
not proof of runtime behavior or a security signature. Runtime checks belong to
the app/MCP acceptance tests. Linux/macOS artifacts are not inferred from a
successful Windows build.

After a maintainer decides to accept the app, the reviewed text patch and the
included original PNG must be applied together: `solvercraft.png` belongs at
`assets/app-icons/solvercraft.png` in the launcher checkout. Then run that
repository's complete required checks. This proposal's isolated check is not a
replacement for those checks.

## Reproducible icon and patch

`generate_icon.py` renders the original numerical-chart geometry as a 128 px PNG
and SVG without reading or copying any external artwork. Both assets and their
recipe are licensed **MIT OR Apache-2.0**, like Solvercraft. Attribution is to
the Solvercraft contributors.

`prepare_submission.py /path/to/reference` rebuilds the patch from the exact
pinned source files plus the source manifest `base-files.json`; it refuses any
changed input blob. It does not fetch or modify upstream. The reference snapshot
used in development is a local inspection aid and is not part of the runtime app.
