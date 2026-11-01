# Binary-oracle request-recording erratum

The historical independent exact-binary v1 report retained a mutable reference
to its third request. Preparing the separate node-limit control later replaced
the stored third request with the maximization/node 1 configuration. The actual
serialized minimization run had already returned -500, but the stored model
was not a faithful reproduction record. Historical final5/v1 artifacts remain
unchanged and are superseded for this provenance question by the corrected v2.

The verifier now deep-copies each request and uses a separate control request.
It reran exactly three objective variants and one node-limit control on the
unchanged release binary 68a6ab8da2646ddb82ca312b6c8977e8c3cf596ed802b8f343e92d51d662c6e1.
All saved request snapshots were checked again after the control run.

| Case | Direction / offset | Expected and actual objective | Point |
|---|---|---:|---|
|0|Maximize /0|500|[0,1]|
|1|Maximize /10000000000000000|10000000000000500|[0,1]|
|2|Minimize /0, negative coefficients|-500|[0,1]|
|Node control|Same model as case 0, max_nodes 1|0, BestFound / node_limit|[0,0]|

The control retains bound 1000.0000000000001 and gap 1.0. No seed was supplied;
the measured effective default is the decimal string 0, not seed 42. The public
[current oracle projection](exact-binary-oracle-final6-report.json) includes
objective context, expected/enumerated points, exact wire/canonical request
hashes, effective options and the model relation for the control.

The corrected full local v2 report has SHA-256
60c193c40707b83632dd2ce2aee7b8fb62f8489c3fb7ae6e1ea06069c12e9afa.
This was a verification-export correction. Product source, the 791 tested
compilation inputs and their accepted gate results were not changed.
