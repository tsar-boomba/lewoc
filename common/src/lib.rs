#![no_std]

pub const DISPLAY_WIDTH: u32 = 128;
pub const DISPLAY_HEIGHT: u32 = 160;

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    strum::AsRefStr,
    num_enum::TryFromPrimitive,
    num_enum::IntoPrimitive,
)]
#[repr(u8)]
pub enum Station {
    #[strum(serialize = "San Francisco")]
    SFKingStreet,
    #[strum(serialize = "22nd Street")]
    SF22ndStreet,
    #[strum(serialize = "Bayshore")]
    Bayshore,
    #[strum(serialize = "Burlingame")]
    Burlingame,
    #[strum(serialize = "San Mateo")]
    SanMateo,
    #[strum(serialize = "San Jose")]
    SanJoseDiridon,
}
