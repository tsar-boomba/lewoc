use embassy_rp::{
    Peri,
    gpio::{self, Input, Output},
    pio, pio_programs,
    spi::{self, ClkPin, MisoPin, MosiPin},
};
use embedded_hal::{self, spi::SpiBus};

pub fn create<'d, T: SpiBus>(
    spi_driver: T,
    cs: Peri<'d, impl gpio::Pin>,
    dc: Peri<'d, impl gpio::Pin>,
    reset: Peri<'d, impl gpio::Pin>,
) {
    let cs = Output::new(cs, embassy_rp::gpio::Level::Low);
    let dc = Output::new(dc, embassy_rp::gpio::Level::Low);
    let reset = Output::new(reset, embassy_rp::gpio::Level::Low);

    let mut display = st7735_lcd::ST7735::new(spi_driver, dc, reset, true, false, 128, 160);
    if let Err(err) = display.init(&mut embassy_time::Delay) {
        log::error!("error setup display: {err:?}")
    }

    log::info!("Clearing Display...");
    display.clear(Rgb565::new(0, 0, 0)).unwrap();

    // graphics::fill_screen(&mut display);
    graphics::draw_message(&mut display, "Hello, World!");
    if let Err(err) = display.update() {
        log::error!("error updating display: {err:?}");
        return;
    };
}
