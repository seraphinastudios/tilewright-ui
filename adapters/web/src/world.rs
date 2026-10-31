// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! Consumer-owned neutral fixture, using the released renderer's procedural
//! test scene and resource provider. No consumer game data or assets are used.
use std::sync::Arc;
use tilewright_render_api::{
    BackendPreference, RenderRequest, Renderer, TargetDescriptor, TargetKind,
};
use tilewright_render_resources::ResourceProvider;
use tilewright_render_testkit::{top_down_world_scene, world_provider};
use tilewright_render_wgpu::WgpuRenderer;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WorldView {
    renderer: WgpuRenderer,
    resources: Arc<dyn ResourceProvider>,
}

#[wasm_bindgen]
impl WorldView {
    pub async fn create(
        canvas: web_sys::HtmlCanvasElement,
        width: u32,
        height: u32,
        backend: &str,
    ) -> Result<WorldView, JsError> {
        let backend = match backend {
            "auto" => BackendPreference::Automatic,
            "webgpu" => BackendPreference::WebGpu,
            "webgl2" => BackendPreference::WebGl2,
            _ => return Err(JsError::new("Invalid backend")),
        };
        let renderer = WgpuRenderer::with_canvas(
            TargetDescriptor {
                kind: TargetKind::Embedded,
                width,
                height,
                backend,
            },
            canvas,
        )
        .await
        .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Self {
            renderer,
            resources: world_provider().map_err(|e| JsError::new(&e.to_string()))?,
        })
    }
    pub fn backend(&self) -> String {
        self.renderer.adapter().backend.clone()
    }
    pub fn render(&mut self, width: u32, height: u32) -> Result<(), JsError> {
        if self.renderer.target().width != width || self.renderer.target().height != height {
            self.renderer
                .resize(width, height)
                .map_err(|e| JsError::new(&e.to_string()))?;
        }
        let scene =
            top_down_world_scene(width, height).map_err(|e| JsError::new(&e.to_string()))?;
        self.renderer
            .present(&RenderRequest {
                scene: &scene,
                resources: self.resources.clone(),
            })
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(())
    }
}
