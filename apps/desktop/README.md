# Desktop application

The Flutter presentation layer for ilikepdf. Run commands from this directory;
repository-wide setup and architecture are documented in the root README and
`docs/architecture.md`. The desktop opens on a tool-card home and routes enabled
tools into a shared workspace/settings layout. PDF-to-Images uses an ordered
multi-PDF card grid, page-1 thumbnails, per-source or custom destinations, batch
progress, PNG/JPG output selection, and mixed-success summaries. PDF-to-Images and Images-to-PDF both call
typed Rust APIs, accept native Windows Explorer drops through `desktop_drop
0.8.4`, share picker/drop ingestion paths, and keep native processing off the
Flutter UI isolate.
