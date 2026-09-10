# Bundled PDFium Runtime

The Windows x64 runtime is vendored so installed builds render PDFs offline and
do not depend on a machine-wide PDFium installation.

- PDFium version: `151.0.7881.0` (`chromium/7881`)
- Distribution: `bblanchon/pdfium-binaries`, Windows x64, non-V8/non-XFA
- Release: <https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium%2F7881>
- Distribution release commit: `867b3ec`
- Artifact: <https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F7881/pdfium-win-x64.tgz>
- Artifact SHA-256: `73cc0de638ac2095e7445bf56a38200a5b7c7ca0e9f4ba144598f2457377ac08`
- `pdfium.dll` SHA-256: `79d4676b656cfb1abcea88f9ade3b4b0826c5200382db5f4ec72a636c598c118`
- Architecture: `windows-x86_64`
- Distributor license: MIT; PDFium: BSD-3-Clause; bundled component notices
  are preserved in `windows/x64/licenses/`.

`pdfium-render` is pinned to the matching `pdfium_7881` API feature. Upgrade
the crate and binary together, verify both hashes, preserve all notices, and run
the native rendering tests plus a Windows release build.
