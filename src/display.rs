use embassy_rp::{
    Peri,
    gpio::{self, Input, Output},
    pio, pio_programs,
    spi::{self, ClkPin, MisoPin, MosiPin},
};
use embedded_hal::spi::SpiDevice;

pub fn create<'d, T: SpiDevice>(
    spi_driver: T,
    dc: Peri<'d, impl gpio::Pin>,
    reset: Peri<'d, impl gpio::Pin>,
) {
    let dc = Output::new(dc, embassy_rp::gpio::Level::Low);
    let reset = Output::new(reset, embassy_rp::gpio::Level::Low);

    let mut display = st7735_lcd::ST7735::new(spi_driver, dc, reset, true, false, 128, 160);
    if let Err(err) = display.init(&mut embassy_time::Delay) {
        log::error!("error setup display: {err:?}")
    }

    graphics::fill(&mut display);

    graphics::draw_message(&mut display, "Hello, World!");
}
