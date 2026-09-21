/* wayland_blur.rs
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use gdk4_wayland::prelude::{WaylandSurfaceExt, WaylandSurfaceExtManual};
use gtk::prelude::*;
use wayland_client::{
    delegate_noop,
    globals::{registry_queue_init, GlobalListContents},
    protocol::{wl_compositor, wl_region, wl_registry},
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum,
};
use wayland_protocols::ext::background_effect::v1::client::{
    ext_background_effect_manager_v1::{
        Capability, Event as ManagerEvent, ExtBackgroundEffectManagerV1,
    },
    ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
};

#[derive(Default)]
struct ProtocolState {
    supports_blur: bool,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for ProtocolState {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtBackgroundEffectManagerV1, ()> for ProtocolState {
    fn event(
        state: &mut Self,
        _: &ExtBackgroundEffectManagerV1,
        event: ManagerEvent,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ManagerEvent::Capabilities { flags } = event {
            state.supports_blur = match flags {
                WEnum::Value(flags) => flags.contains(Capability::Blur),
                WEnum::Unknown(flags) => flags & Capability::Blur.bits() != 0,
            };
        }
    }
}

delegate_noop!(ProtocolState: wl_compositor::WlCompositor);
delegate_noop!(ProtocolState: wl_region::WlRegion);
delegate_noop!(ProtocolState: ExtBackgroundEffectSurfaceV1);

pub struct WaylandBlur {
    connection: Connection,
    event_queue: EventQueue<ProtocolState>,
    state: ProtocolState,
    compositor: wl_compositor::WlCompositor,
    manager: ExtBackgroundEffectManagerV1,
    effect: ExtBackgroundEffectSurfaceV1,
    surface: gdk4_wayland::WaylandSurface,
    wl_surface: wayland_client::protocol::wl_surface::WlSurface,
    blur_region: Option<(i32, i32, i32, i32)>,
    opaque_region: Option<(i32, i32, i32, i32)>,
}

impl WaylandBlur {
    pub fn new(window: &gtk::Window) -> Option<Self> {
        let surface = window
            .surface()?
            .downcast::<gdk4_wayland::WaylandSurface>()
            .ok()?;
        let wl_surface = surface.wl_surface()?;
        let backend = wl_surface.backend().upgrade()?;
        let connection = Connection::from_backend(backend);
        let (globals, mut event_queue) = registry_queue_init::<ProtocolState>(&connection).ok()?;
        let queue_handle = event_queue.handle();
        let compositor = globals
            .bind::<wl_compositor::WlCompositor, _, _>(&queue_handle, 1..=1, ())
            .ok()?;
        let manager = globals
            .bind::<ExtBackgroundEffectManagerV1, _, _>(&queue_handle, 1..=1, ())
            .ok()?;
        globals.destroy();
        let mut state = ProtocolState::default();

        event_queue.roundtrip(&mut state).ok()?;
        if !state.supports_blur {
            return None;
        }

        let effect = manager.get_background_effect(&wl_surface, &queue_handle, ());
        connection.flush().ok()?;

        Some(Self {
            connection,
            event_queue,
            state,
            compositor,
            manager,
            effect,
            surface,
            wl_surface,
            blur_region: None,
            opaque_region: None,
        })
    }

    pub fn update(
        &mut self,
        window: &gtk::Window,
        sidebar: &gtk::Widget,
        content: &gtk::Widget,
        visible: bool,
        collapsed: bool,
    ) -> bool {
        let _ = self.event_queue.dispatch_pending(&mut self.state);

        let blur_region = if visible && !collapsed && self.state.supports_blur {
            sidebar.compute_bounds(window).and_then(|bounds| {
                let x = bounds.x().floor() as i32;
                let y = bounds.y().floor() as i32;
                let right = (bounds.x() + bounds.width()).ceil() as i32;
                let bottom = (bounds.y() + bounds.height()).ceil() as i32;
                let width = right - x;
                let height = bottom - y;

                if width > 0 && height > 0 {
                    Some((x, y, width, height))
                } else {
                    None
                }
            })
        } else {
            None
        };

        let opaque_region = if visible && !collapsed {
            content.compute_bounds(window).and_then(|bounds| {
                let x = bounds.x().floor() as i32;
                let y = bounds.y().floor() as i32;
                let right = (bounds.x() + bounds.width()).ceil() as i32;
                let bottom = (bounds.y() + bounds.height()).ceil() as i32;
                let width = right - x;
                let height = bottom - y;

                (width > 0 && height > 0).then_some((x, y, width, height))
            })
        } else {
            Some((0, 0, window.width(), window.height()))
        };

        if blur_region == self.blur_region && opaque_region == self.opaque_region {
            return self.state.supports_blur;
        }

        let queue_handle = self.event_queue.handle();

        if let Some((x, y, width, height)) = blur_region {
            let wl_region = self.compositor.create_region(&queue_handle, ());
            wl_region.add(x, y, width, height);
            self.effect.set_blur_region(Some(&wl_region));
            wl_region.destroy();
        } else {
            self.effect.set_blur_region(None);
        }

        if let Some((x, y, width, height)) = opaque_region {
            let wl_region = self.compositor.create_region(&queue_handle, ());
            wl_region.add(x, y, width, height);
            self.wl_surface.set_opaque_region(Some(&wl_region));
            wl_region.destroy();
        } else {
            self.wl_surface.set_opaque_region(None);
        }

        self.blur_region = blur_region;
        self.opaque_region = opaque_region;

        let _ = self.connection.flush();
        self.surface.force_next_commit();
        window.queue_draw();

        self.state.supports_blur
    }

    pub fn update_full(&mut self, window: &gtk::Window, enabled: bool) -> bool {
        let _ = self.event_queue.dispatch_pending(&mut self.state);

        let width = window.width();
        let height = window.height();
        let blur_region = (enabled && self.state.supports_blur && width > 0 && height > 0)
            .then_some((0, 0, width, height));
        if blur_region == self.blur_region && self.opaque_region.is_none() {
            return self.state.supports_blur;
        }

        let queue_handle = self.event_queue.handle();
        if let Some((x, y, width, height)) = blur_region {
            let region = self.compositor.create_region(&queue_handle, ());
            region.add(x, y, width, height);
            self.effect.set_blur_region(Some(&region));
            region.destroy();
        } else {
            self.effect.set_blur_region(None);
        }
        self.wl_surface.set_opaque_region(None);
        self.blur_region = blur_region;
        self.opaque_region = None;

        let _ = self.connection.flush();
        self.surface.force_next_commit();
        window.queue_draw();
        self.state.supports_blur
    }
}

impl Drop for WaylandBlur {
    fn drop(&mut self) {
        self.effect.destroy();
        self.manager.destroy();
    }
}
