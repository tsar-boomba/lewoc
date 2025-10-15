use core::{num::NonZeroU128, ops::Range};

use embedded_storage_async::nor_flash::NorFlash;
use sequential_storage::{
    cache::NoCache,
    map::{SerializationError, Value},
};

const DATA_START_ADDR: u32 = 0x0010_0000;
pub const INFO_START_OFFSET: u32 = 0x0;

#[derive(Debug, Clone, Default)]
pub struct Info {
    /// Symmetric encryption key for all packets sent and received. If changed, requires reset of device.
    pub encryption_key: Option<NonZeroU128>,
}

impl Info {
    fn try_from_stored(stored: &StoredInfo) -> Option<Self> {
        Some(Self {
            encryption_key: stored.encryption_key.try_into().ok(),
        })
    }
}


#[derive(Debug, Clone)]
struct StoredInfo {
    encryption_key: u128,
}

impl StoredInfo {
    pub const SER_SIZE: usize = size_of::<u128>();
}

impl<'a> Value<'a> for StoredInfo {
    fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
        if buffer.len() < Self::SER_SIZE {
            return Err(SerializationError::BufferTooSmall);
        }

        // Serialize encryption key first
        buffer[0..size_of::<u128>()].copy_from_slice(&self.encryption_key.to_le_bytes());

        Ok(Self::SER_SIZE)
    }

    fn deserialize_from(buffer: &'a [u8]) -> Result<Self, SerializationError>
    where
        Self: Sized,
    {
        if buffer.len() < Self::SER_SIZE {
            Err(SerializationError::BufferTooSmall)
        } else {
            Ok(Self {
                encryption_key: u128::from_le_bytes(
                    buffer[0..size_of::<u128>()].try_into().unwrap(),
                ),
            })
        }
    }
}

const fn sector_size<S: NorFlash>() -> u32 {
    2 * S::ERASE_SIZE as u32
}

const fn flash_range<S: NorFlash>(offset: u32) -> Range<u32> {
    (DATA_START_ADDR + offset)..((DATA_START_ADDR + offset) + (sector_size::<S>()))
}

pub async fn store_info<S: NorFlash>(
    storage: &mut S,
    info: &Info,
) -> Result<(), sequential_storage::Error<S::Error>> {
    sequential_storage::erase_all(storage, flash_range::<S>(INFO_START_OFFSET)).await?;
    let mut buffer = [0; StoredInfo::SER_SIZE.next_multiple_of(32)];
    let value = StoredInfo {
        encryption_key: info.encryption_key.map_or(0, NonZeroU128::get),
    };

    sequential_storage::map::store_item(
        storage,
        flash_range::<S>(INFO_START_OFFSET),
        &mut NoCache::new(),
        &mut buffer,
        &(),
        &value,
    )
    .await?;
    Ok(())
}

pub async fn load_info<S: NorFlash>(storage: &mut S) -> Option<Info> {
    let mut buffer = [0; StoredInfo::SER_SIZE.next_multiple_of(32)];
    let mut cache = NoCache::new();
    let mut iter = sequential_storage::map::fetch_all_items::<(), _, _>(
        storage,
        flash_range::<S>(INFO_START_OFFSET),
        &mut cache,
        &mut buffer,
    )
    .await
    .ok()?;

    let mut curr_info = None;
    while let Some(((), value)) = iter.next::<StoredInfo>(&mut buffer).await.ok()? {
        curr_info = Some(value);
    }

    curr_info.as_ref().and_then(Info::try_from_stored)
}
