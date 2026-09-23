use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use embassy_stm32::{
    bind_interrupts,
    dma::{InterruptHandler as DmaInterruptHandler},
    exti::{ExtiInput, InterruptHandler as ExtiInterruptHandler},
    gpio::{Output, Level, Speed, Pull},
    i2c::{
        Config as I2cConfig,
        ErrorInterruptHandler as I2cErrorInterruptHandler,
        EventInterruptHandler as I2cEventInterruptHandler,
        I2c,
        mode::Master as I2cMaster
    },
    interrupt,
    mode::Async,
    Peripherals,
    peripherals::{DMA1_CH0, DMA1_CH6, DMA2_CH2, DMA2_CH3, I2C1, USART1, USART2},
    rtc::{Rtc, RtcConfig, RtcTimeProvider},
    spi::{Config as SpiConfig, MODE_0, mode::Master as SpiMaster, Spi},
    time::Hertz,
    usart::{BufferedUart, BufferedInterruptHandler, Config as UsartConfig, DataBits, Parity, StopBits}
};
use static_cell::StaticCell;

const LINKY_BAUD: u32 = 9600;
const SENSOR_BAUD: u32 = 9600;
const ETH_SPI_FREQ: u32 = 1_000_000;
const SD_SPI_FREQ: u32 = 400_000;

pub type SpiPeripheral = Spi<'static, Async, SpiMaster>;
pub type EthernetSpiDevice = ExclusiveDevice<
    SpiPeripheral,
    Output<'static>,
    NoDelay,
>;

static RX_BUF_1: StaticCell<[u8; 64]> = StaticCell::new();
static TX_BUF_1:StaticCell<[u8; 1]> = StaticCell::new();
static RX_BUF_2: StaticCell<[u8; 32]> = StaticCell::new();
static TX_BUF_2:StaticCell<[u8; 1]> = StaticCell::new();

bind_interrupts!(struct Irqs {
    // Exti
    EXTI2 => ExtiInterruptHandler<interrupt::typelevel::EXTI2>;

    // I2c
    I2C1_EV => I2cEventInterruptHandler<I2C1>;
    I2C1_ER => I2cErrorInterruptHandler<I2C1>;
    DMA1_STREAM0 => DmaInterruptHandler<DMA1_CH0>;
    DMA1_STREAM6 => DmaInterruptHandler<DMA1_CH6>;

    // Spi
    EXTI0 => ExtiInterruptHandler<interrupt::typelevel::EXTI0>;
    DMA2_STREAM2 => DmaInterruptHandler<DMA2_CH2>;
    DMA2_STREAM3 => DmaInterruptHandler<DMA2_CH3>;

    // Serial
    USART1 => BufferedInterruptHandler<USART1>;
    USART2 => BufferedInterruptHandler<USART2>;
});

pub struct Board {
    pub buf_usart1: BufferedUart<'static>,
    pub buf_usart2: BufferedUart<'static>,

    pub i2c_dev: I2c<'static, Async, I2cMaster>,
    pub spi_dev: SpiPeripheral,

    pub cs_w5500: Output<'static>,
    pub int_w5500: ExtiInput<'static, Async>,
    pub reset_w5500: Output<'static>,

    pub cs_sd: Output<'static>,

    pub bell: Output<'static>,
    pub button: ExtiInput<'static, Async>,
    pub led: Output<'static>,

    pub rtc: Rtc,
    pub time_provider: RtcTimeProvider,
}

impl Board {
    pub fn init(p: Peripherals) -> Self {
        // Usart
        // linky link
        let mut config = UsartConfig::default();
        // set Linky serial line configuration
        config.baudrate = LINKY_BAUD;
        config.parity = Parity::ParityEven;
        config.data_bits = DataBits::DataBits7;
        config.stop_bits = StopBits::STOP1;

        let rx_buff = RX_BUF_1.init([0u8; 64]);
        let tx_buff = TX_BUF_1.init([0u8; 1]);
        let buf_usart1 = BufferedUart::new(p.USART1, p.PA10, p.PA9, tx_buff, rx_buff, Irqs, config).unwrap();

        // sensor link
        let mut config = UsartConfig::default();
        // set sensor serial line configuration
        config.baudrate = SENSOR_BAUD;
        config.parity = Parity::ParityEven;
        config.data_bits = DataBits::DataBits8;
        config.stop_bits = StopBits::STOP1;

        let rx_buff = RX_BUF_2.init([0u8; 32]);
        let tx_buff = TX_BUF_2.init([0u8; 1]);
        let buf_usart2 = BufferedUart::new(p.USART2, p.PA3, p.PA2, tx_buff, rx_buff, Irqs, config).unwrap();

        // I2c
        let i2c_dev = I2c::new(p.I2C1, p.PB8, p.PB9, p.DMA1_CH6, p.DMA1_CH0, Irqs, I2cConfig::default());

        // Spi
        let spi_dev = Spi::new(
            p.SPI1,
            p.PA5, // SCK
            p.PA7, // MOSI
            p.PA6, // MISO
            p.DMA2_CH3,
            p.DMA2_CH2,
            Irqs,
            SpiConfig::default(),
        );

        // W5500 pins
        let cs_w5500 = Output::new(p.PB3, Level::High, Speed::VeryHigh);
        let int_w5500 = ExtiInput::new(p.PB0, p.EXTI0, Pull::Up, Irqs);
        let reset_w5500 = Output::new(p.PB1, Level::High, Speed::VeryHigh);

        // SD CARD CS (boot phase only)
        let cs_sd = Output::new(p.PB4, Level::High, Speed::VeryHigh);

        let bell = Output::new(p.PB10, Level::Low, Speed::Low);
        let button = ExtiInput::new(p.PB2, p.EXTI2, Pull::None, Irqs);
        let led = Output::new(p.PC13, Level::High, Speed::Low);

        let (rtc, time_provider) = Rtc::new(p.RTC, RtcConfig::default());

        Self {
            buf_usart1,
            buf_usart2,
            i2c_dev,
            spi_dev,
            cs_w5500,
            int_w5500,
            reset_w5500,
            cs_sd,
            bell,
            button,
            led,
            rtc,
            time_provider,
        }
    }
}

pub fn configure_sd(spi: &mut SpiPeripheral) {
    let mut cfg = SpiConfig::default();
    cfg.frequency = Hertz(SD_SPI_FREQ);
    cfg.mode = MODE_0;
    spi.set_config(&cfg).unwrap();
}

pub fn configure_w5500(spi: &mut SpiPeripheral) {
    let mut cfg = SpiConfig::default();
    cfg.frequency = Hertz(ETH_SPI_FREQ);
    cfg.mode = MODE_0;
    spi.set_config(&cfg).unwrap();
}
