use embassy_rp::{
    Peri,
    gpio::{self, Output},
};
use embedded_graphics_coordinate_transform::Rotate90;
use embedded_hal::spi::SpiDevice;

pub struct Display<'d, T: SpiDevice> {
    pub display: Rotate90<st7735_lcd::ST7735<T, Output<'d>, Output<'d>>>,
}

pub enum DisplayMessage {
    None,
    Message(heapless::String<128>),
}

impl<'d, T: SpiDevice> Display<'d, T> {
    pub fn new(
        spi_driver: T,
        dc: Peri<'d, impl gpio::Pin>,
        reset: Peri<'d, impl gpio::Pin>,
    ) -> Self {
        let dc = Output::new(dc, embassy_rp::gpio::Level::Low);
        let reset = Output::new(reset, embassy_rp::gpio::Level::Low);

        let mut display: st7735_lcd::ST7735<T, Output<'_>, Output<'_>> = st7735_lcd::ST7735::new(
            spi_driver,
            dc,
            reset,
            true,
            false,
            common::DISPLAY_WIDTH,
            common::DISPLAY_HEIGHT,
        );

        if let Err(err) = display.init(&mut embassy_time::Delay) {
            log::error!("error setup display: {err:?}");
        }

        let mut display = Rotate90::new(display);

        graphics::fill(&mut display);
        graphics::draw_message(&mut display, "Waiting for hard coded string cause yoni slow wiring");
        Display { display }
    }

    pub fn draw(&mut self, message: &str) {
        graphics::fill(&mut self.display);
        graphics::draw_message(&mut self.display, message);
    }
}
