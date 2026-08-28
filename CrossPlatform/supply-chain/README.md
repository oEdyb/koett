# Supply-chain inventory

This directory defines Koett's draft source inventory. It is not a release
compliance claim. The current Windows and Linux control binaries still contain
the blocked Piper and eSpeak path.

Run:

```sh
python3 scripts/test_supply_chain_inventory.py
python3 scripts/generate_supply_chain_inventory.py --check
```

Run the generator without `--check` after an intentional dependency, model,
native input, bundled asset, or license-policy change. Review every generated
change before committing it.

## Why this is local

Koett needs one inventory across Cargo, SwiftPM, bundled JavaScript, native
archives, and downloaded models. No current official tool covers that exact
set. Swift 6.3.3 also has no `swift package sbom` command. One Python 3.10
standard-library script therefore reads Cargo's stable metadata format,
`Cargo.lock`, SwiftPM's checked-in `Package.resolved`, and this explicit policy.
It adds no app or build dependency.

The generated SBOM uses CycloneDX 1.7. It has a `pre-build` lifecycle and an
`incomplete` composition on purpose. A final release needs a separate
post-build SBOM for each inspected binary.

## Primary references

- [Cargo metadata](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html)
  requires an explicit format version; Koett uses format `1` with `--locked`.
- [Cargo license fields](https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields)
  define `OR` as a user choice. The policy records one selected branch for
  every resolved ambiguous expression.
- [SwiftPM dependency resolution](https://github.com/swiftlang/swift-package-manager/blob/main/Sources/PackageManagerDocs/Documentation.docc/ResolvingPackageVersions.md)
  defines `Package.resolved` as the top-level resolved dependency record.
- [SwiftPM's v3 store source](https://github.com/swiftlang/swift-package-manager/blob/main/Sources/PackageGraph/ResolvedPackagesStore.swift)
  is the current reference for the checked-in file shape.
- [CycloneDX 1.7](https://cyclonedx.org/specification/overview/) is the current
  specification and supports software, native components, and ML models.
- [CycloneDX 1.7 JSON schema](https://github.com/CycloneDX/specification/blob/1.7/schema/bom-1.7.schema.json)
  is the validation contract.
- [Swift SBOM proposal SE-0509](https://github.com/swiftlang/swift-evolution/blob/main/proposals/0509-swift-sboms-via-swiftpm.md)
  informed the Swift component shape, but the installed Swift 6.3.3 tool does
  not implement its command.

Model licenses come from the exact pinned Hugging Face model cards or license
files. Native versions and hashes come from sherpa-onnx `v1.13.5` source at
`3dc7c569f31ca2cd4a20ed6f7db780327e6714c5` and the frozen control report.
The generated gap file remains the authoritative list of missing license and
attribution material.
