// Document module — renders projections of the event log.
//
// Splits into two strategies because .docx (zipped OOXML) can't be cleanly
// templated as text, while .md/.html/.json can.
//
//   - render.rs        — orchestrator + step projection
//   - docx_renderer.rs — programmatic .docx builder driven by a JSON spec
//   - text_renderer.rs — MiniJinja-driven text renderer (.md, .html, .json, .txt)

pub mod docx_renderer;
pub mod render;
pub mod text_renderer;

pub use render::{render_to_file, RenderRequest, StepView};
