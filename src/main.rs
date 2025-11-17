#![no_std]
#![no_main]

mod bt_server;
mod display;
mod input;
mod lora;
mod peri;
mod proto;
mod storage;

use core::num::NonZeroU128;

use embassy_executor::{Executor, Spawner};
use embassy_futures::join;
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::Pull;
use embassy_rp::multicore::{Stack, spawn_core1};
use embassy_rp::{bind_interrupts, gpio, peripherals::USB, usb};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::signal::Signal;
use embassy_sync::zerocopy_channel;
use embassy_time::{Delay, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use gpio::{Input, Level, Output};

use crate::display::DisplayMessage;
use crate::input::Button;
use crate::peri::{Core0Peripherals, Core1Peripherals};
use cyw43_pio::{PioSpi, RM2_CLOCK_DIVIDER};
use embassy_rp::peripherals::{DMA_CH0, PIO0, PIO1};
use embassy_rp::pio::{self, Pio};
use static_cell::{ConstStaticCell, StaticCell};
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
const ID: &str = env!("ID");

static mut CORE1_STACK: Stack<8192> = Stack::new();
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();
static DISPLAY_CHANNEL_DATA: StaticCell<[DisplayMessage; 1]> = StaticCell::new();
static DISPLAY_CHANNEL: StaticCell<
    zerocopy_channel::Channel<'static, CriticalSectionRawMutex, DisplayMessage>,
> = StaticCell::new();

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
async fn input(
    signal: &'static Signal<NoopRawMutex, Button>,
    good_in: Input<'static>,
    help_in: Input<'static>,
) {
    input::task(signal, good_in, help_in).await;
}

#[embassy_executor::task]
async fn core0_main(
    spawner: Spawner,
    sender: zerocopy_channel::Sender<'static, CriticalSectionRawMutex, DisplayMessage>,
    p: Core0Peripherals,
) {
    /// SAFETY: `NoopRawMutex` is ok since we only signal WITHIN core0's executor
    static INPUT_SIGNAL: ConstStaticCell<Signal<NoopRawMutex, Button>> =
        ConstStaticCell::new(Signal::new());
    static BT_MSG_SIGNAL: ConstStaticCell<
        Signal<NoopRawMutex, trouble_host::prelude::HeaplessString<128>>,
    > = ConstStaticCell::new(Signal::new());
    static STATE: StaticCell<cyw43::State> = StaticCell::new();

    // add some delay to give an attached debug probe time to parse the
    // defmt RTT header. Reading that header might touch flash memory, which
    // interferes with flash write operations.
    // https://github.com/knurling-rs/defmt/pull/683
    Timer::after_millis(10).await;

    let driver = usb::Driver::new(p.usb, Irqs);
    spawner.spawn(logger_task(driver).unwrap());

    let fw = cyw43_firmware::CYW43_43439A0;
    let clm = cyw43_firmware::CYW43_43439A0_CLM;
    let bt_fw = cyw43_firmware::CYW43_43439A0_BTFW;

    let pwr = Output::new(p.pin23, Level::Low);
    let cs = Output::new(p.pin25, Level::High);
    let mut pio = Pio::new(p.pio0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        RM2_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        p.pin24,
        p.pin29,
        p.dma0,
    );

    // spawner.spawn(pwm_backlight_task(p.PWM_SLICE1, p.PIN_3).unwrap());
    // spawner.spawn(btn_to_led(btn, light).unwrap());

    let state = STATE.init(cyw43::State::new());
    let (_net_device, bt_device, mut control, runner) =
        cyw43::new_with_bluetooth(state, pwr, spi, fw, bt_fw).await;
    spawner.spawn(cyw43_task(runner).unwrap());

    control.init(clm).await;

    log::info!("Initialized cyw44");

    let controller: ExternalController<_, 10> = ExternalController::new(bt_device);
    let mut flash: embassy_rp::flash::Flash<'_, _, _, FLASH_SIZE> =
        embassy_rp::flash::Flash::new(p.flash, p.dma1);

    let info = storage::load_info(&mut flash)
        .await
        .unwrap_or_else(|| storage::Info {
            encryption_key: DEFAULT_ENCRYPTION_KEY.try_into().ok(),
        });
    log::info!("loaded info: {info:#?}");

    let input_signal = INPUT_SIGNAL.take();
    let bt_msg_signal = BT_MSG_SIGNAL.take();

    spawner.spawn(
        input(
            input_signal,
            Input::new(p.pin6, Pull::Up),
            Input::new(p.pin7, Pull::Up),
        )
        .unwrap(),
    );

    join::join(
        bt_server::run(control, controller, bt_msg_signal, &mut RoscRng, &mut flash),
        // core::future::pending::<()>(),
        lora::run(
            p.spi0,
            p.pin18,
            p.pin19,
            p.pin16,
            p.dma2,
            p.dma3,
            p.pin17,
            p.pin20,
            p.pin22,
            p.pin4,
            &mut RoscRng,
            info.encryption_key
                .map_or(DEFAULT_ENCRYPTION_KEY, NonZeroU128::get),
            input_signal,
            bt_msg_signal,
            sender,
        ),
    )
    .await;

    log::error!("Futures ended!");

    loop {
        cortex_m::asm::wfi();
    }
}

/// Core 1 Executor is dedicated to receiving and handling display drawing
#[embassy_executor::task]
async fn core1_main(
    spawner: Spawner,
    mut receiver: zerocopy_channel::Receiver<'static, CriticalSectionRawMutex, DisplayMessage>,
    p: Core1Peripherals,
) {
    // add some delay to give an attached debug probe time to parse the
    // defmt RTT header. Reading that header might touch flash memory, which
    // interferes with flash write operations.
    // https://github.com/knurling-rs/defmt/pull/683
    Timer::after_millis(10).await;

    let mut pio1 = Pio::new(p.pio1, Irqs);

    let mut config = embassy_rp::spi::Config::default();
    config.frequency = 24_000_000;

    let display_spi = embassy_rp::pio_programs::spi::Spi::new_blocking(
        &mut pio1.common,
        pio1.sm0,
        p.pin28,
        p.pin27,
        p.pin26,
        config,
    );

    let display_spi =
        ExclusiveDevice::new(display_spi, Output::new(p.pin2, Level::High), Delay).unwrap();

    let mut display = display::Display::new(display_spi, p.pin0, p.pin1);
    let mut last_msg_str = heapless::String::<128>::new();

    loop {
        let msg = receiver.receive().await;

        match msg {
            DisplayMessage::None => {}
            DisplayMessage::Message(msg_str) => {
                if last_msg_str != *msg_str {
                    display.draw(msg_str);
                    core::mem::swap(&mut last_msg_str, msg_str);
                }
            }
        }

        receiver.receive_done();
    }
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(embassy_rp::config::Config::default());
    let channel_data = DISPLAY_CHANNEL_DATA.init([DisplayMessage::None]);
    let channel = DISPLAY_CHANNEL.init(zerocopy_channel::Channel::new(channel_data));
    let (sender, receiver) = channel.split();

    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| {
                let main_task = core1_main(
                    spawner,
                    receiver,
                    Core1Peripherals {
                        pio1: p.PIO1,
                        pin0: p.PIN_0,
                        pin1: p.PIN_1,
                        pin2: p.PIN_2,
                        pin26: p.PIN_26,
                        pin27: p.PIN_27,
                        pin28: p.PIN_28,
                    },
                )
                .unwrap();
                spawner.spawn(main_task);
            })
        },
    );

    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| {
        let main_task = core0_main(
            spawner,
            sender,
            Core0Peripherals {
                usb: p.USB,
                flash: p.FLASH,
                spi0: p.SPI0,
                pio0: p.PIO0,
                dma0: p.DMA_CH0,
                dma1: p.DMA_CH1,
                dma2: p.DMA_CH2,
                dma3: p.DMA_CH3,
                pin4: p.PIN_4,
                pin6: p.PIN_6,
                pin7: p.PIN_7,
                pin16: p.PIN_16,
                pin17: p.PIN_17,
                pin18: p.PIN_18,
                pin19: p.PIN_19,
                pin20: p.PIN_20,
                pin22: p.PIN_22,
                pin23: p.PIN_23,
                pin24: p.PIN_24,
                pin25: p.PIN_25,
                pin29: p.PIN_29,
            },
        )
        .unwrap();
        spawner.spawn(main_task);
    })
}
