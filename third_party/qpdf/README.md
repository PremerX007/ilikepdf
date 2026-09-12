# qpdf runtime

iLikePDF bundles the official qpdf 12.4.1 Windows x64 MSVC distribution for
offline structural PDF operations. The Windows archive is relocatable, is the
upstream-recommended high-performance build for 64-bit Windows, and is invoked
directly by Rust without a shell or system `PATH` lookup.

`runtime-manifest.txt` is the canonical source of version, target, artifact,
checksum, required-runtime, license, and provenance metadata. The Windows CMake
packaging step reads that manifest, verifies every listed file and SHA-256, and
fails the build if the pinned distribution is incomplete or changed.

## Provenance

- Version: qpdf 12.4.1
- Platform/architecture: Windows x86_64
- Distribution: `qpdf-12.4.1-msvc64.zip`
- Release: <https://github.com/qpdf/qpdf/releases/tag/v12.4.1>
- Artifact: <https://github.com/qpdf/qpdf/releases/download/v12.4.1/qpdf-12.4.1-msvc64.zip>
- Artifact SHA-256:
  `3cd016cd433ef7232e42f4c13348a49cc14907a3c7278ef4f99120593126f7a6`
- License: Apache License 2.0

The exact upstream checksum manifest and its Sigstore bundle are retained under
`provenance/`. The downloaded archive was hashed locally and matched the entry in
`qpdf-12.4.1.sha256`. Upstream documents verification of the checksum manifest
with `cosign verify-blob` and the accompanying `.sigstore` bundle. Cosign was not
installed in the development environment used for this vendoring step, so the
bundle is preserved for CI or a future provenance audit rather than claimed as
locally verified.

## Bundled files

Only the `qpdf` command and DLLs from the official `bin/` directory are retained.
Development headers, import libraries, manuals, and the auxiliary `fix-qdf` and
`zlib-flate` programs are not required by the application and are omitted.
`LICENSE.txt` and `NOTICE.md` are exact copies from the upstream `v12.4.1` tag.

The release layout is:

```text
runtime/qpdf/
    runtime-manifest.txt
    bin/
        qpdf.exe
        qpdf30.dll
        MSVC runtime DLLs
    licenses/
        LICENSE.txt
        NOTICE.md
    provenance/
        qpdf-12.4.1.sha256
        qpdf-12.4.1.sha256.sigstore
```

Do not replace individual binaries. Upgrade the archive, manifest checksums,
license/notice copies, Rust compatibility expectation, packaging verification,
and integration tests as one reviewed change.
