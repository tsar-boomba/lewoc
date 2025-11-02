#![no_std]
#![no_main]

mod bt_server;
mod display;
mod lora;
mod storage;
mod utils;

use core::num::NonZeroU128;

use embassy_executor::Spawner;
use embassy_futures::join;
use embassy_rp::Peri;
use embassy_rp::clocks::RoscRng;
use embassy_rp::{bind_interrupts, gpio, peripherals::USB, usb};
use embassy_time::{Delay, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use gpio::{Level, Output};

use cyw43_pio::{PioSpi, RM2_CLOCK_DIVIDER};
use embassy_rp::peripherals::{DMA_CH0, PIN_3, PIO0, PIO1, PWM_SLICE1};
use embassy_rp::pio::{self, Pio};
use static_cell::StaticCell;
use trouble_host::prelude::ExternalController;
use {defmt_rtt as _, panic_probe as _};

// Program metadata for `picotool info`.
// This isn't needed, but it's recomended to have these minimal entries.
#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"LEWOC"),
    embassy_rp::binary_info::rp_program_description!(
        c"LoRa Enabled Wireless Off-Grid Communication"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
    PIO1_IRQ_0 => pio::InterruptHandler<PIO1>;
});

const FLASH_SIZE: usize = 4 * 1024 * 1024;
const DEFAULT_ENCRYPTION_KEY: u128 = 0xF22B_4E48_59B3_4D73_9C8D_559B_2C12_2C5D;

#[embassy_executor::task]
async fn logger_task(driver: usb::Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Debug, driver);
}

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn pwm_backlight_task(slice: Peri<'static, PWM_SLICE1>, bl_pin: Peri<'static, PIN_3>) {
    use embassy_rp::pwm::{Config, Pwm};
    use embassy_time::Timer;

    const LEVELS: [f32; 3] = [0.15, 0.50, 1.00];

    let mut cfg = Config::default();
    cfg.top = 32_768;

    let mut pwm = Pwm::new_output_b(slice, bl_pin, cfg.clone());

    let mut index = 0;

    loop {
        let duty = (cfg.top as f32 * LEVELS[index]) as u16;
        cfg.compare_b = duty;
        pwm.set_config(&cfg);

        log::info!(
            "Backlight brightness: {}% ({} / {})",
            (LEVELS[index] * 100.0) as u8,
            duty,
            cfg.top
        );

        Timer::after_secs(2).await;

        index = (index + 1) % LEVELS.len();
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // add some delay to give an attached debug probe time to parse the
    // defmt RTT header. Reading that header might touch flash memory, which
    // interferes with flash write operations.
    // https://github.com/knurling-rs/defmt/pull/683
    Timer::after_millis(10).await;

    let driver = usb::Driver::new(p.USB, Irqs);
    spawner.spawn(logger_task(driver).unwrap());

    // display::create(
    //     p.SPI1, p.PIN_10, p.PIN_11, p.PIN_8, p.PIN_9, p.PIN_13, p.PIN_15,
    //     p.PIN_14,
    // );

    let fw = include_bytes!("../43439A0.bin");
    let clm = include_bytes!("../43439A0_clm.bin");
    let bt_fw = include_bytes!("../43439A0_btfw.bin");

    // To make flashing faster for development, you may want to flash the firmwares independently
    // at hardcoded addresses, instead of baking them into the program with `include_bytes!`:
    //     probe-rs download 43439A0.bin --binary-format bin --chip RP2040 --base-address 0x10100000
    //     probe-rs download 43439A0_clm.bin --binary-format bin --chip RP2040 --base-address 0x10140000
    // let fw = unsafe { core::slice::from_raw_parts(0x10100000 as *const u8, 230321) };
    // let clm = unsafe { core::slice::from_raw_parts(0x10140000 as *const u8, 4752) };

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        RM2_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        p.DMA_CH0,
    );

    let mut pio1 = Pio::new(p.PIO1, Irqs);

    let mut config = embassy_rp::spi::Config::default();
    config.frequency = 24_000_000;

    let display_spi = embassy_rp::pio_programs::spi::Spi::new_blocking(
        &mut pio1.common,
        pio1.sm0,
        p.PIN_28,
        p.PIN_27,
        p.PIN_26,
        config,
    );

    let display_spi =
        ExclusiveDevice::new(display_spi, Output::new(p.PIN_2, Level::High), Delay).unwrap();

    let screen = display::create(display_spi, p.PIN_0, p.PIN_1);

    spawner.spawn(pwm_backlight_task(p.PWM_SLICE1, p.PIN_3).unwrap());

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state: &mut cyw43::State = STATE.init(cyw43::State::new());
    let (_net_device, bt_device, mut control, runner) =
        cyw43::new_with_bluetooth(state, pwr, spi, fw, bt_fw).await;
    spawner.spawn(cyw43_task(runner).unwrap());

    control.init(clm).await;

    log::info!("Initialized cyw44");

    let controller: ExternalController<_, 10> = ExternalController::new(bt_device);
    let mut flash: embassy_rp::flash::Flash<'_, _, _, FLASH_SIZE> =
        embassy_rp::flash::Flash::new(p.FLASH, p.DMA_CH1);

    let info = storage::load_info(&mut flash)
        .await
        .unwrap_or_else(|| storage::Info {
            encryption_key: DEFAULT_ENCRYPTION_KEY.try_into().ok(),
        });
    log::info!("loaded info: {info:#?}");

    join::join(
        bt_server::run(control, controller, &mut RoscRng, &mut flash),
        // core::future::pending::<()>(),
        lora::run(
            p.SPI0,
            p.PIN_18,
            p.PIN_19,
            p.PIN_16,
            p.DMA_CH2,
            p.DMA_CH3,
            p.PIN_17,
            p.PIN_20,
            p.PIN_22,
            p.PIN_4,
            &mut RoscRng,
            info.encryption_key
                .map_or(DEFAULT_ENCRYPTION_KEY, NonZeroU128::get),
        ),
    )
    .await;

    log::error!("Futures ended!");

    loop {}
}
