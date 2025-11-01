use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use embedded_graphics_simulator::{
    BinaryColorTheme, OutputSettingsBuilder, SimulatorDisplay, Window,
};
use std::time::Duration;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let mut display: SimulatorDisplay<Rgb565> =
        SimulatorDisplay::new(Size::new(common::DISPLAY_WIDTH, common::DISPLAY_HEIGHT));

    let output_settings = OutputSettingsBuilder::new()
        .theme(BinaryColorTheme::Default)
        .build();

    let mut window = Window::new("LEWOC Window Sim", &output_settings);
    window.update(&display);

    graphics::draw_message(&mut display, "Hey Andria!");
    window.update(&display);

    loop {
        for event in window.events() {
            match event {
                embedded_graphics_simulator::SimulatorEvent::Quit => std::process::exit(0),
                _ => {}
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}
