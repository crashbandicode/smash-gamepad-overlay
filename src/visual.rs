use skyline::nn::ui2d::{Layout, Pane, PaneFlag};
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::{OverlayConfig, OVERLAY_CONFIG};
use crate::input::{ControllerSnapshot, ControllerViewState};
use crate::logger::trace;
use crate::skin::{BuiltInSkin, SkinElement, PRO_CONTROLLER_STATIC_SKIN};
use crate::ui::find_pane_by_name;

pub(crate) const MAX_RESOLVED_SKIN_ELEMENTS: usize = 32;

#[derive(Debug, Copy, Clone)]
pub(crate) enum VisualRenderError {
    MissingSkinPane {
        skin_name: &'static str,
        pane_name: &'static [u8],
    },
}

#[derive(Debug, Copy, Clone)]
struct ResolvedSkin {
    layout: *mut Layout,
    layout_root: *mut Pane,
    skin_name: &'static str,
    skin_root: *mut Pane,
    panes: [*mut Pane; MAX_RESOLVED_SKIN_ELEMENTS],
    pane_count: usize,
}

#[derive(Debug, Copy, Clone)]
enum VisualCache {
    Empty,
    Resolved(ResolvedSkin),
    Missing {
        layout: *mut Layout,
        layout_root: *mut Pane,
        skin_name: &'static str,
        error: VisualRenderError,
    },
}

struct VisualRuntime {
    cache: VisualCache,
}

struct VisualRuntimeCell(UnsafeCell<VisualRuntime>);

// The draw hook touches this cache from Smash's UI draw path only.
unsafe impl Sync for VisualRuntimeCell {}

impl VisualRuntime {
    const fn new() -> Self {
        Self {
            cache: VisualCache::Empty,
        }
    }

    unsafe fn render(
        &mut self,
        layout: *mut Layout,
        layout_root: *mut Pane,
        skin: &'static BuiltInSkin,
        state: &ControllerViewState,
    ) -> Result<(), VisualRenderError> {
        if !self.cache_matches(layout, layout_root, skin) {
            self.cache = VisualCache::Empty;
        }

        match self.cache {
            VisualCache::Resolved(mut resolved) => {
                if !resolved.is_valid_for(layout, layout_root, skin) {
                    self.cache = VisualCache::Empty;
                    return self.resolve_and_render(layout, layout_root, skin, state);
                }

                update_resolved_skin(&mut resolved, skin, state);
                self.cache = VisualCache::Resolved(resolved);
                Ok(())
            }
            VisualCache::Missing { error, .. } => Err(error),
            VisualCache::Empty => self.resolve_and_render(layout, layout_root, skin, state),
        }
    }

    unsafe fn resolve_and_render(
        &mut self,
        layout: *mut Layout,
        layout_root: *mut Pane,
        skin: &'static BuiltInSkin,
        state: &ControllerViewState,
    ) -> Result<(), VisualRenderError> {
        match resolve_skin(layout, layout_root, skin) {
            Ok(mut resolved) => {
                update_resolved_skin(&mut resolved, skin, state);
                self.cache = VisualCache::Resolved(resolved);

                if !VISUAL_PANES_LOGGED.swap(true, Ordering::Relaxed) {
                    trace(&format!("visual mode using injected skin '{}'", skin.name));
                }

                Ok(())
            }
            Err(error) => {
                self.cache = VisualCache::Missing {
                    layout,
                    layout_root,
                    skin_name: skin.name,
                    error,
                };
                Err(error)
            }
        }
    }

    fn cache_matches(
        &self,
        layout: *mut Layout,
        layout_root: *mut Pane,
        skin: &BuiltInSkin,
    ) -> bool {
        match self.cache {
            VisualCache::Empty => false,
            VisualCache::Resolved(resolved) => resolved.is_for(layout, layout_root, skin),
            VisualCache::Missing {
                layout: cached_layout,
                layout_root: cached_layout_root,
                skin_name,
                ..
            } => {
                cached_layout == layout
                    && cached_layout_root == layout_root
                    && skin_name == skin.name
            }
        }
    }
}

impl ResolvedSkin {
    fn new(
        layout: *mut Layout,
        layout_root: *mut Pane,
        skin: &BuiltInSkin,
        skin_root: *mut Pane,
    ) -> Self {
        Self {
            layout,
            layout_root,
            skin_name: skin.name,
            skin_root,
            panes: [std::ptr::null_mut(); MAX_RESOLVED_SKIN_ELEMENTS],
            pane_count: 0,
        }
    }

    fn push(&mut self, pane: *mut Pane) {
        self.panes[self.pane_count] = pane;
        self.pane_count += 1;
    }

    fn is_for(&self, layout: *mut Layout, layout_root: *mut Pane, skin: &BuiltInSkin) -> bool {
        self.layout == layout && self.layout_root == layout_root && self.skin_name == skin.name
    }

    fn is_valid_for(
        &self,
        layout: *mut Layout,
        layout_root: *mut Pane,
        skin: &BuiltInSkin,
    ) -> bool {
        self.is_for(layout, layout_root, skin)
            && !self.skin_root.is_null()
            && self.pane_count == skin.elements.len()
            && self.panes[..self.pane_count]
                .iter()
                .all(|pane| !pane.is_null())
    }
}

static VISUAL_PANES_LOGGED: AtomicBool = AtomicBool::new(false);
static VISUAL_RUNTIME: VisualRuntimeCell = VisualRuntimeCell(UnsafeCell::new(VisualRuntime::new()));

pub(crate) unsafe fn render_visual_overlay(
    layout: *mut Layout,
    root_pane: *mut Pane,
    snapshot: Option<ControllerSnapshot>,
) -> Result<(), VisualRenderError> {
    let view_state = snapshot
        .map(ControllerViewState::from_snapshot)
        .unwrap_or_else(ControllerViewState::neutral);
    (*VISUAL_RUNTIME.0.get()).render(layout, root_pane, &PRO_CONTROLLER_STATIC_SKIN, &view_state)
}

unsafe fn resolve_skin(
    layout: *mut Layout,
    root_pane: *mut Pane,
    skin: &'static BuiltInSkin,
) -> Result<ResolvedSkin, VisualRenderError> {
    let skin_root = find_named_pane(root_pane, skin, skin.root_pane_name)?;
    let mut resolved = ResolvedSkin::new(layout, root_pane, skin, skin_root);

    for element in skin.elements.iter().take(MAX_RESOLVED_SKIN_ELEMENTS) {
        let pane = find_skin_pane(skin_root, skin, element)?;
        resolved.push(pane);
    }

    if resolved.pane_count != skin.elements.len() {
        let missing_element = skin
            .elements
            .get(resolved.pane_count)
            .unwrap_or(&skin.elements[skin.elements.len() - 1]);
        return Err(VisualRenderError::MissingSkinPane {
            skin_name: skin.name,
            pane_name: missing_element.pane_name,
        });
    }

    Ok(resolved)
}

unsafe fn update_resolved_skin(
    resolved: &mut ResolvedSkin,
    skin: &BuiltInSkin,
    state: &ControllerViewState,
) {
    update_visual_skin_root(resolved.skin_root);

    for (pane, element) in resolved.panes[..resolved.pane_count]
        .iter()
        .zip(skin.elements.iter())
    {
        update_visual_skin_pane(*pane, element, state);
    }
}

unsafe fn find_skin_pane(
    root_pane: *mut Pane,
    skin: &BuiltInSkin,
    element: &SkinElement,
) -> Result<*mut Pane, VisualRenderError> {
    find_named_pane(root_pane, skin, element.pane_name)
}

unsafe fn find_named_pane(
    root_pane: *mut Pane,
    skin: &BuiltInSkin,
    pane_name: &'static [u8],
) -> Result<*mut Pane, VisualRenderError> {
    let pane = find_pane_by_name(root_pane, pane_name);
    if pane.is_null() {
        Err(VisualRenderError::MissingSkinPane {
            skin_name: skin.name,
            pane_name,
        })
    } else {
        Ok(pane)
    }
}

pub(crate) unsafe fn update_visual_skin_root(pane: *mut Pane) {
    update_visual_skin_root_with_config(pane, &OVERLAY_CONFIG);
}

pub(crate) unsafe fn update_visual_skin_root_with_config(pane: *mut Pane, config: &OverlayConfig) {
    (*pane).set_visible(true);
    (*pane).pos_x = config.x;
    (*pane).pos_y = config.y;
    (*pane).pos_z = 0.0;
    (*pane).scale_x = config.scale;
    (*pane).scale_y = config.scale;
    (*pane).alpha = config.opacity;
    (*pane).global_alpha = config.opacity;
    (*pane).flags |= 1 << PaneFlag::IsGlobalMatrixDirty as u8;
}

pub(crate) unsafe fn hide_visual_skin_root(pane: *mut Pane) {
    if pane.is_null() {
        return;
    }

    (*pane).alpha = 0;
    (*pane).global_alpha = 0;
    (*pane).set_visible(false);
    (*pane).flags |= 1 << PaneFlag::IsGlobalMatrixDirty as u8;
}

pub(crate) unsafe fn update_visual_skin_pane(
    pane: *mut Pane,
    element: &SkinElement,
    state: &ControllerViewState,
) {
    let value = state.control_value(element.control_id);
    let visible = if value.pressed {
        element.pressed_visible
    } else {
        element.released_visible
    };
    let alpha = if value.pressed {
        interpolate_alpha(element.released_alpha, element.pressed_alpha, value.analog)
    } else {
        element.released_alpha
    };
    let element_scale = if value.pressed {
        interpolate_scale(element.released_scale, element.pressed_scale, value.analog)
    } else {
        element.released_scale
    };
    let (stick_x, stick_y) = state
        .stick_position(element.control_id)
        .zip(element.stick_movement_radius)
        .map(|((x, y), radius)| (x * radius, y * radius))
        .unwrap_or((0.0, 0.0));

    (*pane).set_visible(visible);
    (*pane).pos_x = scaled(element.base_x + stick_x);
    (*pane).pos_y = scaled(element.base_y + stick_y);
    (*pane).pos_z = 0.0;
    (*pane).scale_x = element_scale;
    (*pane).scale_y = element_scale;
    (*pane).size_x = element.size_x;
    (*pane).size_y = element.size_y;
    (*pane).alpha = alpha;
    (*pane).global_alpha = alpha;
    (*pane).flags |= 1 << PaneFlag::IsGlobalMatrixDirty as u8;
}

fn interpolate_alpha(released_alpha: u8, pressed_alpha: u8, analog: f32) -> u8 {
    let analog = analog.clamp(0.0, 1.0);
    (released_alpha as f32 + (pressed_alpha as f32 - released_alpha as f32) * analog)
        .clamp(0.0, 255.0) as u8
}

fn interpolate_scale(released_scale: f32, pressed_scale: f32, analog: f32) -> f32 {
    let analog = analog.clamp(0.0, 1.0);
    released_scale + (pressed_scale - released_scale) * analog
}

fn scaled(value: f32) -> f32 {
    value * OVERLAY_CONFIG.scale
}
