use embassy_rp::{
    Peri,
    peripherals::{
        DMA_CH0, DMA_CH1, DMA_CH2, DMA_CH3, FLASH, PIN_0, PIN_1, PIN_2, PIN_4, PIN_6, PIN_7,
        PIN_16, PIN_17, PIN_18, PIN_19, PIN_20, PIN_22, PIN_23, PIN_24, PIN_25, PIN_26, PIN_27,
        PIN_28, PIN_29, PIO0, PIO1, SPI0, USB,
    },
};

pub struct Core0Peripherals {
    pub usb: Peri<'static, USB>,
    pub flash: Peri<'static, FLASH>,
    pub spi0: Peri<'static, SPI0>,
    pub pio0: Peri<'static, PIO0>,
    pub dma0: Peri<'static, DMA_CH0>,
    pub dma1: Peri<'static, DMA_CH1>,
    pub dma2: Peri<'static, DMA_CH2>,
    pub dma3: Peri<'static, DMA_CH3>,
    pub pin4: Peri<'static, PIN_4>,
    pub pin6: Peri<'static, PIN_6>,
    pub pin7: Peri<'static, PIN_7>,
    pub pin16: Peri<'static, PIN_16>,
    pub pin17: Peri<'static, PIN_17>,
    pub pin18: Peri<'static, PIN_18>,
    pub pin19: Peri<'static, PIN_19>,
    pub pin20: Peri<'static, PIN_20>,
    pub pin22: Peri<'static, PIN_22>,
    pub pin23: Peri<'static, PIN_23>,
    pub pin24: Peri<'static, PIN_24>,
    pub pin25: Peri<'static, PIN_25>,
    pub pin29: Peri<'static, PIN_29>,
}

pub struct Core1Peripherals {
    pub pio1: Peri<'static, PIO1>,
    pub pin0: Peri<'static, PIN_0>,
    pub pin1: Peri<'static, PIN_1>,
    pub pin2: Peri<'static, PIN_2>,
    pub pin26: Peri<'static, PIN_26>,
    pub pin27: Peri<'static, PIN_27>,
    pub pin28: Peri<'static, PIN_28>,
}
