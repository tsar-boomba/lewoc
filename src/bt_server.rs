use embassy_futures::join::join;
use embedded_storage_async::nor_flash::NorFlash;
use rand_core::{CryptoRng, RngCore};
use trouble_host::prelude::*;

use crate::storage::{Info, load_info};

/// Max number of connections
const CONNECTIONS_MAX: usize = 1;

/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att

// GATT Server definition
#[gatt_server]
struct Server {
    service: CustomService,
}

// TODO: create new service and characteristics; share code between FE and FW
const SERVICE_UUID: u128 = 0xB41C9628F8C34F2B80054AC70A4A06BB;
const CHARACTERISTIC_UUID: u128 = 0xEEC428D7C63E4AE897FA3F300C236452;

#[gatt_service(uuid = SERVICE_UUID)]
struct CustomService {
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "station", read, value = "Current Station")]
    #[characteristic(uuid = CHARACTERISTIC_UUID, read, write, value = 255)]
    characteristic: u8,
}

/// Run the BLE stack.
pub async fn run<C, RNG, S>(
    mut control: cyw43::Control<'static>,
    controller: C,
    random_generator: &mut RNG,
    storage: &mut S,
) where
    C: Controller,
    RNG: RngCore + CryptoRng,
    S: NorFlash,
{
    // Using a fixed "random" address can be useful for testing. In real scenarios, one would
    // use e.g. the MAC 6 byte array as the address (how to get that varies by the platform).
    let address: Address = Address::random(control.address().await);

    log::info!("Our address = {}", address);

    let mut info = if let Some(stored_info) = load_info(storage).await {
        log::info!("got stored info");
        stored_info
    } else {
        log::info!("using default info");
        Info::default()
    };

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(address)
        .set_random_generator_seed(random_generator)
        .set_io_capabilities(IoCapabilities::DisplayOnly);

    let Host {
        mut peripheral,
        runner,
        ..
    } = stack.build();

    log::info!("Starting advertising and GATT service");
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "Caltrain Bike Tag",
        appearance: &appearance::DISPLAY,
    }))
    .unwrap();

    let _ = join(ble_task(runner), async {
        loop {
            control.gpio_set(0, true).await;
            match advertise(&mut peripheral, &server).await {
                Ok(conn) => {
                    // set up tasks when the connection is established to a central, so they don't run when no one is connected.
                    gatt_events_task(&mut control, storage, &mut info, &server, &conn)
                        .await
                        .unwrap();
                }
                Err(e) => {
                    #[cfg(feature = "defmt")]
                    let e = defmt::Debug2Format(&e);
                    panic!("[adv] error: {:?}", e);
                }
            }
        }
    })
    .await;
}

/// This is a background task that is required to run forever alongside any other BLE tasks.
///
/// ## Alternative
///
/// If you didn't require this to be generic for your application, you could statically spawn this with i.e.
///
/// ```rust,ignore
///
/// #[embassy_executor::task]
/// async fn ble_task(mut runner: Runner<'static, SoftdeviceController<'static>>) {
///     runner.run().await;
/// }
///
/// spawner.must_spawn(ble_task(runner));
/// ```
async fn ble_task<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(e) = runner.run().await {
            #[cfg(feature = "defmt")]
            let e = defmt::Debug2Format(&e);
            panic!("[ble_task] error: {:?}", e);
        }
    }
}

/// Stream Events until the connection closes.
///
/// This function will handle the GATT events and process them.
/// This is how we interact with read and write requests.
async fn gatt_events_task<S: NorFlash>(
    control: &mut cyw43::Control<'static>,
    storage: &mut S,
    info: &mut Info,
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, DefaultPacketPool>,
) -> Result<(), Error> {
    let characteristic = server.service.characteristic;

    let reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::PairingComplete { security_level, .. } => {
                log::info!("[gatt] pairing complete: {:?}", security_level);
            }
            GattConnectionEvent::PairingFailed(err) => {
                log::error!("[gatt] pairing error: {:?}", err);
            }
            GattConnectionEvent::Gatt { event } => {
                let result = match &event {
                    GattEvent::Read(event) => {
                        if event.handle() == characteristic.handle {
                            let value = server.get(&characteristic);
                            log::info!("[gatt] Read Event to Characteristic: {:?}", value);
                        }
                        None
                    }
                    GattEvent::Write(event) => {
                        if event.handle() == characteristic.handle {
                            let value = event.value(&characteristic).unwrap();
                            log::info!("[gatt] Write to Characteristic: {}", value);
                        }

                        None
                    }
                    _ => None,
                };

                let reply_result = if let Some(code) = result {
                    log::info!("[gatt] Rejected GATT event");
                    event.reject(code)
                } else {
                    log::info!("[gatt] Accepted GATT event");
                    event.accept()
                };

                match reply_result {
                    Ok(reply) => reply.send().await,
                    Err(e) => log::warn!("[gatt] error sending response: {:?}", e),
                }

                log::info!("[gatt] Sent GATT reply");
            }
            _ => log::info!("[gatt] Other GATT event ignored"), // ignore other Gatt Connection Events
        }
    };

    log::info!("[gatt] disconnected: {:?}", reason);
    Ok(())
}

/// Create an advertiser to use to connect to a BLE Central, and wait for it to connect.
async fn advertise<'values, 'server, C: Controller>(
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    server: &'server Server<'values>,
) -> Result<GattConnection<'values, 'server, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids128(&[SERVICE_UUID.to_le_bytes()]),
        ],
        &mut advertiser_data[..],
    )?;
    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..len],
                scan_data: &[],
            },
        )
        .await?;
    log::info!("[adv] advertising");
    let conn = advertiser.accept().await?.with_attribute_server(server)?;
    log::info!("[adv] connection established");
    Ok(conn)
}
