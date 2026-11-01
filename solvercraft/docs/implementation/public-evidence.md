# Public verification evidence

The public diagnostics are compact, fixed-field projections of actual executed
runs. They retain test counts and outcomes, selected numerical case results,
per-process performance measurements, effective options and seeds, software
versions, source/binary fingerprints and public package names/version/SPDX.
Full local reports and arbitrary stdout/stderr, command lines, environment,
filesystem paths, driver/session details and repeated manifest dumps are omitted.

[public-evidence-projection.json](public-evidence-projection.json) records each
projection and its public SHA-256. `full_local_report_sha256` identifies the
retained pre-projection full copy; this may already have undergone path-only
redaction. Original unredacted run archives remain local. These hashes do not
mean the full local report is included in the repository.

Public text logs contain only recognized test identifiers/results, `running`
counts and final test summaries. Other original lines are blank. Line counts
and retained test positions are preserved, so the 111 mathematical and 19 PSI
signature evidence locators remain checkable. Compiler-only transcripts can
therefore be blank; their exit status is in the corresponding gate projection.
The excerpts are not presented as full original build transcripts.

The [redaction ledger](evidence-redactions.json) distinguishes original/path-only
and current public-copy identities. The final6 source delta provides five
source changes against the already published final5 source manifest; the tested
791 source/build/data files and separate worker-test fingerprint are unchanged.
Raw and LF-canonical source identities remain distinct. Frozen planning,
reference and numeric-oracle input bytes retain their recorded identities.

The product's structured run-manifest API is unchanged. These small public
verification projections are evidence, not build inputs or complete API dumps.
Rerun the checked-in tests/harnesses to create full local reports. The two worker
attempts remain separate: initial 7/1 with 26 ms, one identical repeat 8/0 with 12 ms;
there is no aggregate all-attempts-pass or hard 25 ms guarantee.

The [binary-oracle export erratum](binary-oracle-export-erratum.md) explains the
corrected request-recording metadata and the fresh 3+1 check. License details are
in [the checked-in third-party notices](../../licenses/THIRD_PARTY.md), with
public package/version/SPDX inventories linked from the verification summary.
