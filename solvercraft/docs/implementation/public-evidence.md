# Public verification evidence

Published diagnostic reports and logs use portable path placeholders. Local
filesystem paths, profile/device identifiers, ephemeral service addresses and
nonessential environment values are omitted. Hardware dimensions requested by
the performance criterion, tool versions, seeds, tolerances, measured outcomes,
times, memory/error measurements and source/binary hashes are retained.

Redaction changes publication metadata, not the measured numerical results.
An `accepted` gate still refers to the original executed run and its unchanged
input identity. Public report copies document evidence; rerun the checked-in
harnesses to create fresh local reports usable as build/transport inputs.

Source identity records both raw working-tree and LF-canonical hashes. Published
Rust sources use LF. Frozen planning/reference/oracle data retain their recorded
raw bytes; `.gitattributes` protects these inputs from automatic normalization.
In particular, `exact_products.txt` retains raw SHA-256
`322362fe5fcc15e3a8bde984cbb90e22cded6d49949ee80ffa5d9761e2d0e491`.

The public reference/implementation matrices and original 96-criterion CSV are
unchanged by diagnostic redaction. Remaining function/default/import gaps stay
explicit in the acceptance and reference documents.
