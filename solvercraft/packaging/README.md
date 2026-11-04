# Local Solvercraft distribution

The Windows portable package contains `solvercraft.exe`, `solvercraft-cli.exe`
and `solvercraft-mcp.exe` at its root, with examples, licenses and binary hashes.
The app is a native workbook workbench; the MCP process is launched on demand by
a local client. There is no hosted service or automatic network listener.

## Build a Windows package

Use Rust 1.95+, PowerShell 7, Python 3.11+ and the pinned Craft font inputs documented by the
repository. `CRAFT_FONTS_DIR` must refer to that font checkout. From the repository
root:

```powershell
./solvercraft/packaging/windows/package.ps1
```

The script builds the native application and the two SDK programs with their
locked dependencies. It writes
`dist/solvercraft-0.14.2-windows-x64-portable.zip` and `dist/SHA256SUMS.txt` for the
current application workspace version. Existing builds can be packaged with
`-SkipBuild -TargetDir /absolute/path/to/target -OutputDir /absolute/path/to/output`.
The font checkout remains required when skipping the build so the embedded
fonts' license notices are included. Other applications' checksum rows in an
existing sums file are preserved; Solvercraft's matching asset row is updated.
Dependency notices are collected from both locked workspace graphs, including
Git-hosted UI crates, rather than relying on the historical SDK-only inventory.
The resolved inventory contains public package/version/SPDX/source fields only.
For published crates that omit their license files, the packager verifies
checked-in notices pinned to the package's exact upstream VCS revision. Font
notices and the SDK's separately licensed numerical data are also included.
Staging directories are retained under the target directory for inspection.

The application follows Gridcraft's workspace version; the independent SDK and
its tools keep their own version. The application package version must match the
GitHub release version used by the launcher.

## GitHub and catalog delivery

Source changes belong on the existing Solvercraft branch/PR. Binary archives are
release artifacts, not source files to add to Git. Publishing a GitHub release
requires attaching the actual portable ZIP and its `SHA256SUMS.txt`; a source push
alone does not publish downloadable application releases.

The proposed [ArtCraft Launcher patch](../integrations/artcraft-launcher/README.md)
is pinned to its real catalog contract and requires maintainer review. Since
Gridcraft and Solvercraft currently share a repository/release feed, every
launcher-visible release must carry both applications' expected assets. A future
separate repository would require an explicit catalog source update. Do not
advertise an asset or publisher identity before it exists.

macOS signing and Linux portable builds are separate platform checks; the
Windows package is not evidence that those targets have been executed.
