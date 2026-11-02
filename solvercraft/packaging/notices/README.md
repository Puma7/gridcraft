# Pinned dependency notices

These are original license and copyright-notice files missing from the resolved
crate archives used by the Windows Solvercraft package. They supplement the
notices copied directly from available crate source directories; they do not
replace that complete resolved-dependency inventory.

`manifest.json` records each exact package name/version, the original public raw
source URL at its immutable VCS commit and the SHA-256 of every included file.
The VCS commits were obtained from the resolved packages' `.cargo_vcs_info.json`.
Every distinct source file was also checked against its GitHub Git blob ID before
the package-specific copies were made. File bytes are preserved unchanged.

The four AccessKit crates share one upstream commit and include its original MIT,
Apache, Chromium BSD and author notices. `gl_generator` and `khronos_api` use their
different recorded VCS revisions; the former `brendanzab/gl-rs` repository has
moved to `rust-windowing/gl-rs`, verified as the same GitHub repository ID 7655767.
The URLs record its current canonical owner at those unchanged commits.

No generic replacement license text or invented copyright attribution is used.
Any added package/version requires its own verified source and manifest entry.
