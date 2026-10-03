use super::error::{ApplicationError, ApplicationErrorCode};
use flutter_rust_bridge::frb;
use ilikepdf_core as core;
use std::path::Path;

#[frb(opaque)]
pub struct EditorSession {
    inner: core::EditorSession,
}
#[frb(opaque)]
pub struct EditorLayoutBinding {
    inner: core::EditorDocumentLayout,
    session_id: u64,
}
#[frb(opaque)]
pub struct EditorPageTransform {
    inner: core::PageTransform,
}

#[derive(Clone)]
pub struct EditorRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone)]
pub struct EditorPoint {
    pub x: f64,
    pub y: f64,
}
#[derive(Clone)]
pub struct EditorPageBox {
    pub left: f64,
    pub bottom: f64,
    pub right: f64,
    pub top: f64,
}
#[derive(Clone, Copy)]
pub enum EditorPageRotation {
    None,
    Clockwise90,
    HalfTurn,
    Clockwise270,
}
#[derive(Clone)]
pub struct EditorPageGeometry {
    pub visible_box: EditorPageBox,
    pub declared_media_box: Option<EditorPageBox>,
    pub declared_crop_box: Option<EditorPageBox>,
    pub rotation: EditorPageRotation,
}
#[frb(non_opaque)]
pub struct EditorPageLayout {
    pub page_index: u32,
    pub rect: EditorRect,
    pub scale: f64,
    pub geometry: EditorPageGeometry,
    pub transform: EditorPageTransform,
}
#[frb(non_opaque)]
pub struct EditorDocumentLayout {
    pub binding: EditorLayoutBinding,
    pub pages: Vec<EditorPageLayout>,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}
pub struct EditorHit {
    pub page_index: u32,
    pub pdf_point: EditorPoint,
    pub page_local_point: EditorPoint,
}
#[derive(Clone, Copy)]
pub struct EditorRasterSize {
    pub width: u32,
    pub height: u32,
}
pub struct EditorRaster {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub fn open_editor_session(source_path: String) -> Result<EditorSession, ApplicationError> {
    core::open_editor_session(Path::new(&source_path))
        .map(|inner| EditorSession { inner })
        .map_err(Into::into)
}

impl EditorSession {
    #[frb(sync)]
    pub fn identity(&self) -> u64 {
        self.inner.id().value()
    }
    #[frb(sync)]
    pub fn page_count(&self) -> u32 {
        self.inner.page_count()
    }
    /// Pure geometry; native rendering remains asynchronous on the worker pool.
    #[frb(sync)]
    pub fn layout(
        &self,
        workspace_width: f64,
        scale: f64,
        fit_width: bool,
    ) -> Result<EditorDocumentLayout, ApplicationError> {
        let mode = if fit_width {
            core::EditorZoomMode::FitWidth
        } else {
            core::EditorZoomMode::Custom(core::EditorZoom::new(scale).map_err(geometry_error)?)
        };
        let inner = core::EditorDocumentLayout::new(&self.inner, workspace_width, mode)
            .map_err(geometry_error)?;
        let pages = inner
            .pages()
            .iter()
            .map(|page| {
                let rect = page.rect();
                let geometry = page.geometry;
                EditorPageLayout {
                    page_index: page.page_index,
                    rect: EditorRect {
                        left: rect.left(),
                        top: rect.top(),
                        width: rect.width(),
                        height: rect.height(),
                    },
                    scale: page.zoom.scale(),
                    geometry: EditorPageGeometry {
                        visible_box: map_box(geometry.visible_box()),
                        declared_media_box: geometry.declared_media_box().map(map_box),
                        declared_crop_box: geometry.declared_crop_box().map(map_box),
                        rotation: match geometry.rotation() {
                            core::PageRotation::None => EditorPageRotation::None,
                            core::PageRotation::Clockwise90 => EditorPageRotation::Clockwise90,
                            core::PageRotation::HalfTurn => EditorPageRotation::HalfTurn,
                            core::PageRotation::Clockwise270 => EditorPageRotation::Clockwise270,
                        },
                    },
                    transform: EditorPageTransform {
                        inner: page.transform,
                    },
                }
            })
            .collect();
        Ok(EditorDocumentLayout {
            width: inner.width(),
            height: inner.height(),
            scale: inner.zoom().scale(),
            pages,
            binding: EditorLayoutBinding {
                inner,
                session_id: self.inner.id().value(),
            },
        })
    }
    pub fn render(
        &self,
        page_index: u32,
        size: EditorRasterSize,
    ) -> Result<EditorRaster, ApplicationError> {
        let size = core::EditorRenderSize::new(size.width, size.height).map_err(geometry_error)?;
        core::render_editor_page(&self.inner, page_index, size)
            .map(|raster| EditorRaster {
                png: raster.png,
                width: raster.size.width(),
                height: raster.size.height(),
            })
            .map_err(Into::into)
    }
}

impl EditorLayoutBinding {
    #[frb(sync)]
    pub fn hit_test(&self, point: EditorPoint, scroll: EditorPoint) -> Option<EditorHit> {
        self.inner
            .hit_test(viewport_point(point), viewport_point(scroll))
            .map(|hit| EditorHit {
                page_index: hit.page_index,
                pdf_point: EditorPoint {
                    x: hit.pdf_point.x,
                    y: hit.pdf_point.y,
                },
                page_local_point: map_point(hit.page_local_point),
            })
    }
    #[frb(sync)]
    pub fn visible_pages(&self, top: f64, height: f64, overscan: f64) -> Vec<u32> {
        self.inner.visible_pages(top, height, overscan)
    }
    #[frb(sync)]
    pub fn render_size(
        &self,
        page_index: u32,
        density: f64,
    ) -> Result<EditorRasterSize, ApplicationError> {
        let page = self
            .inner
            .pages()
            .get(page_index as usize)
            .ok_or_else(|| ApplicationError {
                code: ApplicationErrorCode::PageOutOfBounds,
                message: "The requested editor page does not exist".into(),
            })?;
        let size = page.render_size(density).map_err(geometry_error)?;
        Ok(EditorRasterSize {
            width: size.width(),
            height: size.height(),
        })
    }
    #[frb(sync)]
    pub fn anchored_scroll(
        &self,
        previous: &EditorLayoutBinding,
        scroll: EditorPoint,
        viewport: EditorPoint,
    ) -> Result<EditorPoint, ApplicationError> {
        if self.session_id != previous.session_id {
            return Err(geometry_error(
                core::PageGeometryError::InvalidDisplayRectangle,
            ));
        }
        self.inner
            .anchored_scroll(
                &previous.inner,
                viewport_point(scroll),
                viewport_point(viewport),
            )
            .map(map_point)
            .map_err(geometry_error)
    }
}

impl EditorPageTransform {
    #[frb(sync)]
    pub fn pdf_to_document(&self, point: EditorPoint) -> Result<EditorPoint, ApplicationError> {
        self.inner
            .pdf_to_viewport(core::PdfPoint {
                x: point.x,
                y: point.y,
            })
            .map(map_point)
            .map_err(geometry_error)
    }
}

#[frb(sync)]
pub fn step_editor_zoom(scale: f64, increase: bool) -> Result<f64, ApplicationError> {
    core::EditorZoom::new(scale)
        .map(|zoom| zoom.stepped(increase).scale())
        .map_err(geometry_error)
}

fn viewport_point(point: EditorPoint) -> core::ViewportPoint {
    core::ViewportPoint {
        x: point.x,
        y: point.y,
    }
}
fn map_point(point: core::ViewportPoint) -> EditorPoint {
    EditorPoint {
        x: point.x,
        y: point.y,
    }
}
fn map_box(rect: core::PageBox) -> EditorPageBox {
    EditorPageBox {
        left: rect.left_points(),
        bottom: rect.bottom_points(),
        right: rect.right_points(),
        top: rect.top_points(),
    }
}
fn geometry_error(_: core::PageGeometryError) -> ApplicationError {
    ApplicationError {
        code: ApplicationErrorCode::InvalidRequest,
        message: "The editor viewport geometry is invalid".into(),
    }
}
