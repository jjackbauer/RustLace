# Which RustLace version does this documentation describe?

The checked-in guides, examples and API reference describe **the source revision
you have checked out**, not every binary published in the past. The documentation
CI executes scripts against a runner built from that same revision.

## Choose a baseline before copying an example

- **Working from source:** check out the desired commit or tag, build
  \`lovelace-run\` there with \`cargo build -p lovelace-run\`, then follow the
  checked-in documentation at that revision. The four tutorials are tested
  against the checked-out tree by \`bash tools/run-tutorials.sh\`.
- **Using a downloadable desktop release:** read the documentation and release
  notes **at that release tag**, not \`main\`. Release assets may not expose
  features present in newer development source.
- **Embedding the engine or consuming JSON:** pin the commit/release and check
  \`protocolVersion\`, \`mathIrVersion\`, \`symbolicFormatVersion\`, and the
  live \`--function\` descriptors. Do not use a workspace version number as a
  substitute for protocol compatibility.
- **Following an agent-generated script:** have the agent report the target
  revision, its inspected descriptor, the exact command, and the result's
  numerical provenance. Reject undocumented assumptions about release parity.

## A concrete compatibility difference

The Systems \`exact_values\` sidecar was introduced after v0.7.1.
A v0.7.1 desktop does not have the member on its \`StationAnalysis\` output.
The code on the current development branch has it; do not copy that example
into a v0.7.1 application and interpret the missing-member failure as an
arithmetic error. Consult the [Systems reference](extensions/systems.md)
for the current contract and check the release's source before relying on it.

## What the test results mean

- The language and extension example runners demonstrate that the **current
  binary** and recorded example output agree.
- The tutorial runner independently verifies selected arithmetic but does not
  prove all mathematical functions correct.
- Historical measurements in [testing](testing.md) name their originating
  commit, host and command. They are not automatically reclassified as results
  for the current revision.
- Passing current CI says nothing about a different, previously released
  binary unless that exact binary or tag was tested.

If you need a reproducible result, retain the commit SHA, command, inputs,
precision settings, typed value and exactness metadata with the output.
