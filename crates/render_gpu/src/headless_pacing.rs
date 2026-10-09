//! GPU back-pressure for a process without a window.
//!
//! A windowed frame waits for a swapchain image, which caps how far the CPU
//! may run ahead of the GPU. Headless has no swapchain and a continuous loop,
//! so nothing caps it: the loop ran at well over a thousand frames a second,
//! every frame queued GPU work, and Bevy's render diagnostics open one Metal
//! counter sample buffer per frame still in flight until the device refused
//! one and was lost. Here the render frame waits for its own submissions
//! instead, which gives headless the one-frame bound a swapchain gives a
//! window.

use bevy::prelude::*;
use bevy::render::renderer::RenderDevice;
use bevy::render::{Extract, ExtractSchedule, Render, RenderApp, RenderSystems};

pub(crate) fn register_headless_pacing(app: &mut App) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .add_systems(ExtractSchedule, extract_headless)
        .add_systems(
            Render,
            wait_for_submitted_frame
                .run_if(resource_exists::<frame::Headless>)
                .after(RenderSystems::Render)
                .before(RenderSystems::Cleanup),
        );
}

fn extract_headless(
    mut commands: Commands,
    main: Extract<Option<Res<frame::Headless>>>,
    render: Option<Res<frame::Headless>>,
) {
    match (main.is_some(), render.is_some()) {
        (true, false) => commands.insert_resource(frame::Headless),
        (false, true) => commands.remove_resource::<frame::Headless>(),
        _ => {}
    }
}

fn wait_for_submitted_frame(device: Res<RenderDevice>) {
    if let Err(error) = device.poll(bevy::render::render_resource::PollType::wait_indefinitely()) {
        diag::warn!(
            World,
            "headless pacing: waiting for this frame's GPU work failed: {error}"
        );
    }
}
