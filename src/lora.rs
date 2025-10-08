use core::ops::Range;

use ascon_aead::{
    AsconAead128,
    aead::{AeadInPlace, KeyInit},
};
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
use static_cell::StaticCell;

use crate::utils::random_u32_in_range;

// warning: set these appropriately for the region
const LORAWAN_REGION: region::Region = region::Region::US915;
const TX_POWER: i32 = 14;
const LORA_FREQUENCY_IN_HZ: u32 = 915_000_000;

const MAX_PAYLOAD_LEN: usize = 222;
const MAC_SIZE: usize = 16;
const NONCE_SIZE: usize = 16;
const MAX_MSG_LEN: usize = MAX_PAYLOAD_LEN - MAC_SIZE - NONCE_SIZE;

const RANDOM_SLEEP_RANGE: Range<u32> = 3..8;
const TRANSMIT_PKT_TIMES: usize = 2;

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
    encryption_key: u128,
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
    let mut lora = LoRa::new(Sx127x::new(spi, iv, config), false, Delay)
        .await
        .unwrap();

    if let Err(err) = lora.init().await {
        log::error!("Error LoRa init: {err:?}");
        return;
    };

    static RECV_BUF: StaticCell<ascon_aead::aead::heapless::Vec<u8, MAX_PAYLOAD_LEN>> =
        StaticCell::new();
    static SEND_BUF: StaticCell<ascon_aead::aead::heapless::Vec<u8, MAX_PAYLOAD_LEN>> =
        StaticCell::new();
    let recv_buf = RECV_BUF.init_with(|| Default::default());
    let send_buf = SEND_BUF.init_with(|| Default::default());

    let key_bytes = encryption_key.to_le_bytes();
    let key = ascon_aead::AsconAead128Key::from_slice(&key_bytes);
    let cipher = ascon_aead::AsconAead128::new(key);
    let nonce = generate_nonce(rng);

    cipher.encrypt_in_place(&nonce, &[], send_buf);

    let mdltn_params = {
        match lora.create_modulation_params(
            SpreadingFactor::_8,
            Bandwidth::_125KHz,
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
            recv_buf.len() as u8,
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

    let ready_to_send = false;

    // Try to receive for a while, then send, and loop doing this
    log::info!("LoRa rx tx loop starting");
    loop {
        let channel_is_active = match lora.cad(&mdltn_params).await {
            Ok(channel_active) => channel_active,
            Err(err) => {
                log::error!("Error checking channel activity: {err:?}");
                Timer::after_millis(random_u32_in_range(rng, RANDOM_SLEEP_RANGE) as u64).await;
                continue;
            }
        };

        if channel_is_active {
            // It takes less energy to check for channel activity, so only do a receive if the channel is active
            match receive(&mut lora, &mdltn_params, &rx_pkt_params, recv_buf).await {
                Ok(None) => {
                    log::debug!("RX timed out")
                }
                Ok(Some(num_read)) => {
                    log::debug!("RX'd {num_read} bytes")
                }
                Err(err) => log::error!("Error rx: {err:?}"),
            }

            if let Err(err) = decrypt_in_place(&cipher, recv_buf) {
                log::error!("Error decrypting packet: {err:?}");
            } else {
                // use received packet through recv_buf
                log::info!("Received packet: {recv_buf:?}");
            };
        } else if ready_to_send {
            // Only try and send if the channel is inactive, and we have something to send

            // TODO: Write real data to send buf
            send_buf.clear();
            send_buf.extend_from_slice(b"Hello LoRa").unwrap();

            if let Ok(_) = encrypt_in_place(&cipher, rng, send_buf) {
                match send(&mut lora, &mdltn_params, &mut tx_pkt_params, send_buf).await {
                    Ok(_) => {
                        log::debug!("sent out pkt")
                    }
                    Err(err) => log::error!("Error tx: {err:?}"),
                }
            } else {
                log::error!("Didn't send packet due to encryption error");
            }
        }

        Timer::after_millis(random_u32_in_range(rng, RANDOM_SLEEP_RANGE) as u64).await;
    }
}

async fn send(
    lora: &mut LoRa<impl RadioKind, impl DelayNs>,
    modulation_params: &ModulationParams,
    packet_params: &mut PacketParams,
    buf: &[u8],
) -> Result<(), RadioError> {
    for _ in 0..TRANSMIT_PKT_TIMES {
        match lora
            .prepare_for_tx(&modulation_params, packet_params, TX_POWER, buf)
            .await
        {
            Ok(()) => {}
            Err(err) => {
                log::error!("Prepare TX error: {err:?}");
                return Err(err);
            }
        };

        log::debug!("LoRa tx-ing");

        lora.tx().await?;
    }

    Ok(())
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

/// Encrypts the contents of `buf` in-place.
///
/// After a successful call, `buf` will have structure: `CIPHERTEXT | MAC (16-bytes) | NONCE (16-bytes)`
fn encrypt_in_place<const N: usize>(
    cipher: &AsconAead128,
    rng: &mut impl RngCore,
    buf: &mut ascon_aead::aead::heapless::Vec<u8, N>,
) -> ascon_aead::aead::Result<()> {
    if buf.capacity() - buf.len() < 32 {
        log::error!("encrypt buf too small for data, mac, and nonce");
        return Err(ascon_aead::Error);
    }

    let nonce = generate_nonce(rng);
    cipher.encrypt_in_place(&nonce, &[], buf)?;
    buf.extend_from_slice(&nonce).unwrap();

    Ok(())
}

/// Decrypts the contents of `buf` in-place. At call-time, buf should contain: CIPHERTEXT | MAC (16-bytes) | NONCE (16-bytes)
///
/// After this function is successful, `buf` will contain the plaintext data.
fn decrypt_in_place<const N: usize>(
    cipher: &AsconAead128,
    buf: &mut ascon_aead::aead::heapless::Vec<u8, N>,
) -> ascon_aead::aead::Result<()> {
    if buf.len() < 32 {
        log::error!("Invalid decrypt buf len: {}", buf.len());
        return Err(ascon_aead::Error);
    }

    let tag_pos = buf.len() - 32;
    let (ciphertext, tag_and_nonce) = buf.split_at_mut(tag_pos);
    let (tag, nonce) = tag_and_nonce.split_at_mut(16);

    cipher.decrypt_in_place_detached(
        ascon_aead::AsconAead128Nonce::from_slice(nonce),
        &[],
        ciphertext,
        ascon_aead::Tag::<AsconAead128>::from_slice(tag),
    )?;
    buf.truncate(tag_pos);

    Ok(())
}

fn generate_nonce(rng: &mut impl RngCore) -> ascon_aead::AsconAead128Nonce {
    let mut bytes = [0; 16];
    rng.fill_bytes(&mut bytes);
    ascon_aead::AsconAead128Nonce::clone_from_slice(&bytes)
}
