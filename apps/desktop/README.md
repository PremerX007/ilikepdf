# Desktop application

The Flutter presentation layer for ilikepdf. Run commands from this directory;
repository-wide setup and architecture are documented in the root README and
`docs/architecture.md`. The desktop opens on a tool-card home and routes enabled
tools into a shared workspace/settings layout. PDF-to-Images and Images-to-PDF
both call typed Rust APIs and keep native processing off the Flutter UI isolate.
Images-to-PDF accepts native Windows Explorer drops through `desktop_drop 0.8.4`;
drop and picker paths share the same workflow ingestion method.
