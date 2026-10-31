// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT
use std::sync::Arc;
use tilewright_render_resources::{Error, ResourceBlob, ResourceProvider, ResourceRequest};
use tilewright_render_ui::{UiTexture, UnicodeTexture, builtin};
#[derive(Clone)]
pub(crate) struct Resources {
    textures: Vec<UnicodeTexture>,
    solid: UiTexture,
    world: Option<Arc<dyn ResourceProvider>>,
}
impl Resources {
    pub(crate) fn new(textures: Vec<UnicodeTexture>, solid: UiTexture) -> Self {
        Self {
            textures,
            solid,
            world: None,
        }
    }
    pub(crate) fn with_world(&self, world: Arc<dyn ResourceProvider>) -> Self {
        Self {
            world: Some(world),
            ..self.clone()
        }
    }
}
impl ResourceProvider for Resources {
    fn load(&self, request: ResourceRequest) -> Result<ResourceBlob, Error> {
        request.validate()?;
        let expected = self
            .textures
            .iter()
            .find(|texture| texture.identity.id == request.id)
            .map(|texture| (texture.identity, texture.bytes.as_ref()))
            .or_else(|| {
                (request.id == self.solid.id).then_some((self.solid, builtin::SOLID_BYTES))
            });
        if let Some((identity, bytes)) = expected {
            if request.revision != identity.revision || request.digest != identity.digest {
                return Err(Error::DigestMismatch);
            }
            if bytes.len() > request.maximum_bytes {
                return Err(Error::LimitExceeded);
            }
            return ResourceBlob::for_request(request, bytes.to_vec());
        }
        self.world.as_ref().ok_or(Error::NotFound)?.load(request)
    }
}
