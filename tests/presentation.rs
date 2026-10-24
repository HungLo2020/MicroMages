//! Presentation regressions: Quasar viewport scaling and Vulkan framebuffer output.
//!
//! The GPU and window checks are ignored by default. CI runs them under Xvfb with
//! Mesa's lavapipe Vulkan driver.
#![cfg(feature = "desktop")]
use micro_mages_re::display::{ASPECT, DisplaySettings, Scaling};
use quasar_core::{Filter, Scaling as QuasarScaling, Viewport};
use std::time::Duration;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 240;

fn options(scaling: Scaling) -> quasar_core::DisplayOptions {
    DisplaySettings {
        scaling,
        ..DisplaySettings::default()
    }
    .options()
}

fn viewport(width: u32, height: u32, scaling: Scaling) -> Viewport {
    Viewport::calculate((WIDTH, HEIGHT), (width, height), options(scaling))
        .expect("drawable target")
}

/// Distinct left/right and top/bottom markers so orientation errors are visible.
fn frame() -> Vec<u8> {
    let mut rgba = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let index = ((y * WIDTH + x) * 4) as usize;
            let color = match (x < 8, y < 8, x >= WIDTH - 8, y >= HEIGHT - 8) {
                (true, _, _, _) => [255, 0, 0, 255],
                (_, true, _, _) => [0, 255, 0, 255],
                (_, _, true, _) => [0, 0, 255, 255],
                (_, _, _, true) => [255, 255, 0, 255],
                _ => [(x % 256) as u8, (y % 256) as u8, 128, 255],
            };
            rgba[index..index + 4].copy_from_slice(&color);
        }
    }
    rgba
}

#[test]
fn every_window_shape_keeps_16_15_with_bars_on_one_axis_only() {
    assert_eq!(ASPECT, (16, 15));
    for width in [256u32, 300, 640, 800, 960, 1024, 1280, 1920, 2560, 3840] {
        for height in [240u32, 300, 480, 600, 720, 1000, 1080, 1200, 1440, 2160] {
            for scaling in [Scaling::Fit, Scaling::Smooth] {
                let view = viewport(width, height, scaling);
                assert!(view.width > 0 && view.height > 0, "{width}x{height}");
                assert!(view.width <= width && view.height <= height);
                // Maximized: one axis fills the target exactly.
                assert!(
                    view.width == width || view.height == height,
                    "{width}x{height}: {view:?}"
                );
                // Bars never appear on all four sides.
                assert!(view.x == 0 || view.y == 0, "{width}x{height}: {view:?}");
                let ratio = f64::from(view.width) / f64::from(view.height);
                assert!(
                    (ratio - 16.0 / 15.0).abs() < 0.02,
                    "{width}x{height}: {ratio}"
                );
            }
        }
    }
}

#[test]
fn wide_and_tall_targets_letterbox_the_expected_axis() {
    // 16:9: full height, equal side bars.
    let wide = viewport(1920, 1080, Scaling::Fit);
    assert_eq!(
        (wide.x, wide.y, wide.width, wide.height),
        (384, 0, 1152, 1080)
    );
    // Portrait: full width, equal top and bottom bars.
    let tall = viewport(600, 1000, Scaling::Smooth);
    assert_eq!(
        (tall.x, tall.y, tall.width, tall.height),
        (0, 219, 600, 562)
    );
    // Exact aspect: no bars at all.
    let exact = viewport(1280, 1200, Scaling::Fit);
    assert_eq!(
        (exact.x, exact.y, exact.width, exact.height),
        (0, 0, 1280, 1200)
    );
    // Smaller than the framebuffer still fits without cropping.
    let small = viewport(128, 120, Scaling::Fit);
    assert_eq!(
        (small.x, small.y, small.width, small.height),
        (0, 0, 128, 120)
    );
    assert_eq!(
        Viewport::calculate((WIDTH, HEIGHT), (0, 0), options(Scaling::Fit)),
        None
    );
}

#[test]
fn filtering_never_changes_the_viewport() {
    for (width, height) in [(1920, 1080), (600, 1000), (1280, 1200)] {
        let sharp = viewport(width, height, Scaling::Fit);
        let smooth = viewport(width, height, Scaling::Smooth);
        assert_eq!(
            (sharp.x, sharp.y, sharp.width, sharp.height),
            (smooth.x, smooth.y, smooth.width, smooth.height)
        );
    }
    assert_eq!(options(Scaling::Fit).filter, Filter::Nearest);
    assert_eq!(options(Scaling::Smooth).filter, Filter::Linear);
    assert_eq!(options(Scaling::Fit).scaling, QuasarScaling::Fit);
}

#[test]
#[ignore = "requires a Vulkan 1.3 driver"]
fn vulkan_framebuffer_round_trip_is_pixel_exact() {
    use quasar_render_2d::{FramebufferPresenter, Renderer2D};
    let renderer = Renderer2D::headless().expect("Vulkan 1.3 renderer");
    let mut presenter = FramebufferPresenter::from_renderer(renderer, (WIDTH, HEIGHT))
        .expect("framebuffer presenter");
    let pixels = frame();
    let rendered = presenter.render_to_image(&pixels).expect("GPU readback");
    assert_eq!(rendered, pixels, "the presenter must copy pixels exactly");
}

#[test]
#[ignore = "requires a display and a Vulkan 1.3 driver"]
fn native_window_presents_the_framebuffer() {
    use quasar_render_2d::FramebufferPresenter;
    use quasar_window::winit::platform::{
        wayland::EventLoopBuilderExtWayland, x11::EventLoopBuilderExtX11,
    };
    use quasar_window::{
        GameWindow, WindowConfig,
        winit::{
            application::ApplicationHandler,
            event::WindowEvent,
            event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        },
    };

    #[derive(Default)]
    struct App {
        presented: bool,
        error: Option<String>,
    }
    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let config = WindowConfig {
                title: "Micro Mages presentation test".into(),
                application_id: "micromages-presentation-test".into(),
                size: (512, 480),
                ..WindowConfig::default()
            };
            let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                let window = GameWindow::create(event_loop, &config)?;
                let size = window.native().inner_size();
                let mut presenter = FramebufferPresenter::new(
                    window.shared_native(),
                    (size.width.max(1), size.height.max(1)),
                    (WIDTH, HEIGHT),
                    options(Scaling::Fit),
                )?;
                presenter.present(&frame())?;
                self.presented = presenter.viewport().is_some();
                drop(presenter);
                Ok(())
            })();
            if let Err(error) = result {
                self.error = Some(error.to_string());
            }
            event_loop.exit();
        }
        fn window_event(
            &mut self,
            _event_loop: &ActiveEventLoop,
            _window_id: quasar_window::winit::window::WindowId,
            _event: WindowEvent,
        ) {
        }
        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + Duration::from_millis(50),
            ));
            event_loop.exit();
        }
    }

    // libtest runs tests on a worker thread; allow the native event loop there.
    let mut builder = EventLoop::builder();
    EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
    EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
    let event_loop = builder.build().expect("event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("event loop ran");
    assert_eq!(app.error, None);
    assert!(app.presented, "no viewport was reported for the window");
}
