use embassy_rp::{
    Peri,
    dma::Channel,
    gpio::{self, Input, Output},
    spi::{self, ClkPin, MisoPin, MosiPin},
};
use embassy_time::{Delay, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::{
    DelayNs,
    mod_params::{ModulationParams, PacketParams, RadioError},
    mod_traits::RadioKind,
    sx127x::{self, Sx1276},
};
use lora_phy::{LoRa, iv::GenericSx127xInterfaceVariant, sx127x::Sx127x};
use lora_phy::{
    RxMode,
    mod_params::{Bandwidth, CodingRate, SpreadingFactor},
};
use lorawan_device::async_device::region;
use rand_core::RngCore;

use crate::utils::random_u32_in_range;

// warning: set these appropriately for the region
const LORAWAN_REGION: region::Region = region::Region::US915;
const TX_POWER: i32 = 14;
const LORA_FREQUENCY_IN_HZ: u32 = 915_000_000;

pub async fn run<'d, T: spi::Instance>(
    spi_peri: Peri<'d, T>,
    clk: Peri<'d, impl ClkPin<T> + 'd>,
    mosi: Peri<'d, impl MosiPin<T> + 'd>,
    miso: Peri<'d, impl MisoPin<T> + 'd>,
    tx_dma: Peri<'d, impl Channel + 'd>,
    rx_dma: Peri<'d, impl Channel + 'd>,
    cs: Peri<'d, impl gpio::Pin>,
    rst: Peri<'d, impl gpio::Pin>,
	dio0: Peri<'d, impl gpio::Pin>,
    dio1: Peri<'d, impl gpio::Pin>,
    rng: &mut impl RngCore,
) {
    let mut config = spi::Config::default();
    config.frequency = 1_000_000; // Maybe use higher frequency on final board if we make one
    let spi = spi::Spi::new(spi_peri, clk, mosi, miso, tx_dma, rx_dma, config);
    let spi = ExclusiveDevice::new(spi, Output::new(cs, gpio::Level::High), Delay).unwrap();

    let config = sx127x::Config {
        chip: Sx1276,
        rx_boost: false,
        tcxo_used: false,
        tx_boost: false,
    };
    let iv = GenericSx127xInterfaceVariant::new(
        Output::new(rst, gpio::Level::High),
        Input::new(dio0, gpio::Pull::None),
		Input::new(dio1, gpio::Pull::None),
        None,
        None,
    )
    .unwrap();
    let mut lora = LoRa::new(Sx127x::new(spi, iv, config), true, Delay)
        .await
        .unwrap();

    if let Err(err) = lora.init().await {
        log::error!("Error LoRa init: {err:?}");
        return;
    };

    let mut receiving_buffer = [0u8; 256];

    let mdltn_params = {
        match lora.create_modulation_params(
            SpreadingFactor::_12,
            Bandwidth::_250KHz,
            CodingRate::_4_5,
            LORA_FREQUENCY_IN_HZ,
        ) {
            Ok(mp) => mp,
            Err(err) => {
                log::info!("Radio error: {err:?}");
                return;
            }
        }
    };

    let rx_pkt_params = {
        match lora.create_rx_packet_params(
            4,
            false,
            receiving_buffer.len() as u8,
            true,
            false,
            &mdltn_params,
        ) {
            Ok(pp) => pp,
            Err(err) => {
                log::info!("Radio error: {err:?}");
                return;
            }
        }
    };

    let mut tx_pkt_params = {
        match lora.create_tx_packet_params(4, false, true, false, &mdltn_params) {
            Ok(pp) => pp,
            Err(err) => {
                log::info!("Radio error: {err:?}");
                return;
            }
        }
    };

    // Try to receive for a while, then send, and loop doing this
    log::info!("LoRa rx tx loop starting");
    loop {
        match send(&mut lora, &mdltn_params, &mut tx_pkt_params, &[1, 2, 3]).await {
            Ok(_) => {
                log::info!("sent out pkt")
            }
            Err(err) => log::error!("Error tx: {err:?}"),
        }

        match receive(
            &mut lora,
            &mdltn_params,
            &rx_pkt_params,
            &mut receiving_buffer,
        )
        .await
        {
            Ok(None) => {
                log::info!("RX timed out")
            }
            Ok(Some(num_read)) => {
                log::info!("RX'd {num_read} bytes")
            }
            Err(err) => log::error!("Error rx: {err:?}"),
        }

        Timer::after_millis(random_u32_in_range(rng, 1..10) as u64).await;
    }
}

async fn send(
    lora: &mut LoRa<impl RadioKind, impl DelayNs>,
    modulation_params: &ModulationParams,
    packet_params: &mut PacketParams,
    buf: &[u8],
) -> Result<(), RadioError> {
    match lora
        .prepare_for_tx(&modulation_params, packet_params, TX_POWER, buf)
        .await
    {
        Ok(()) => {}
        Err(err) => {
            log::info!("Prepare TX error: {err:?}");
            return Err(err);
        }
    };

    log::info!("LoRa tx-ing");

    lora.tx().await
}

async fn receive(
    lora: &mut LoRa<impl RadioKind, impl DelayNs>,
    modulation_params: &ModulationParams,
    packet_params: &PacketParams,
    buf: &mut [u8],
) -> Result<Option<usize>, RadioError> {
    match lora
        .prepare_for_rx(RxMode::Single(128), modulation_params, packet_params)
        .await
    {
        Ok(()) => {}
        Err(err) => {
            log::info!("Prepare RX error: {err:?}");
            return Err(err);
        }
    };

    log::info!("LoRa rx-ing");

    match lora.rx(&packet_params, buf).await {
        Ok((received_len, _rx_pkt_status)) => {
            if (received_len == 3) && (buf[0] == 0x1u8) && (buf[1] == 0x2u8) && (buf[2] == 0x3u8) {
                log::info!("rx successful");
                Ok(Some(0))
            } else {
                log::info!("rx unknown packet");
                Ok(None)
            }
        }
        Err(err) if err == RadioError::ReceiveTimeout => Ok(None),
        Err(err) => Err(err),
    }
}
