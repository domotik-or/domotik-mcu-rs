use embassy_stm32::{
    bind_interrupts,
    can::{Can, CanConfigurator, IT0InterruptHandler, IT1InterruptHandler},
    dma::{InterruptHandler as DmaInterruptHandler},
    exti::{ExtiInput, InterruptHandler as ExtiInterruptHandler},
    gpio::{Output, Level, Speed, Pull},
    i2c::{
        ErrorInterruptHandler as I2cErrorInterruptHandler,
        EventInterruptHandler as I2cEventInterruptHandler,
        I2c,
        mode::Master as I2cMaster
    },
    interrupt,
    mode::Async,
    Peripherals,
    peripherals::{FDCAN1, GPDMA1_CH0, GPDMA1_CH1, GPDMA1_CH2, GPDMA1_CH3, I2C1, RNG, USART1},
    rng::{InterruptHandler as RngInterruptHandler, Rng },
    rtc::{Rtc, RtcConfig, RtcTimeProvider},
    spi::{Spi, Config as SpiConfig, mode::Master as SpiMaster},
    time::Hertz,
    usart::{BufferedUart, BufferedInterruptHandler, Config as UsartConfig, DataBits, Parity, StopBits}
};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    mutex::Mutex
};
use static_cell::StaticCell;

pub type SpiBus = Mutex<
    CriticalSectionRawMutex,
    Spi<'static, Async, SpiMaster>,
>;

const SPI_FREQ: u32 = 9_000_000;
const LINKY_BAUD: u32 = 9600;
const CAN_BITRATE: u32 = 125_000;

static RX_BUF: StaticCell<[u8; 32]> = StaticCell::new();
static TX_BUF: StaticCell<[u8; 32]> = StaticCell::new();
static SPI_BUS: StaticCell<SpiBus> = StaticCell::new();

bind_interrupts!(struct Irqs {
    // Can
    FDCAN1_IT0 => IT0InterruptHandler<FDCAN1>;
    FDCAN1_IT1 => IT1InterruptHandler<FDCAN1>;

    // Spi
    EXTI0 => ExtiInterruptHandler<interrupt::typelevel::EXTI0>;
    GPDMA1_CHANNEL0 => DmaInterruptHandler<GPDMA1_CH0>;
    GPDMA1_CHANNEL1 => DmaInterruptHandler<GPDMA1_CH1>;

    // Exti
    EXTI2 => ExtiInterruptHandler<interrupt::typelevel::EXTI2>;

    // I2c
    I2C1_EV => I2cEventInterruptHandler<I2C1>;
    I2C1_ER => I2cErrorInterruptHandler<I2C1>;
    GPDMA1_CHANNEL2 => DmaInterruptHandler<GPDMA1_CH2>;
    GPDMA1_CHANNEL3 => DmaInterruptHandler<GPDMA1_CH3>;

    // Random Number Generator
    RNG => RngInterruptHandler<RNG>;

    // Serial
    USART1 => BufferedInterruptHandler<USART1>;
});

pub struct Board {
    pub buf_usart: BufferedUart<'static>,
    pub can_bus: Can<'static>,
    pub i2c_bus: I2c<'static, Async, I2cMaster>,
    pub spi_bus: &'static SpiBus,

    pub cs_w5500: Output<'static>,
    pub int_w5500: ExtiInput<'static, Async>,
    pub reset_w5500: Output<'static>,

    pub cs_sd: Output<'static>,

    pub bell: Output<'static>,
    pub button: ExtiInput<'static, Async>,
    pub led: Output<'static>,

    pub rng: Rng<'static, RNG>,
    pub rtc: (Rtc, RtcTimeProvider),
}

impl Board {
    pub fn init(p: Peripherals) -> Self {
        // Usart
        let mut config = UsartConfig::default();
        // set Linky serial line configuration
        config.baudrate = LINKY_BAUD;
        config.parity = Parity::ParityEven;
        config.data_bits = DataBits::DataBits7;
        config.stop_bits =  StopBits::STOP1;

        let rx_buff = RX_BUF.init([0u8; 32]);
        let tx_buff = TX_BUF.init([0u8; 32]);
        let buf_usart = BufferedUart::new(p.USART1, p.PA10, p.PA9, tx_buff, rx_buff, Irqs, config).unwrap();

        // I2c
        let i2c_bus = I2c::new(p.I2C1, p.PB8, p.PB7, p.GPDMA1_CH2, p.GPDMA1_CH3, Irqs, Default::default());

        // Spi
        let mut spi_cfg = SpiConfig::default();
        spi_cfg.frequency = Hertz(SPI_FREQ);

        let spi = Spi::new(
            p.SPI1,
            p.PA5, // SCK
            p.PA7, // MOSI
            p.PA6, // MISO
            p.GPDMA1_CH0,
            p.GPDMA1_CH1,
            Irqs,
            spi_cfg,
        );

        let spi_bus = SPI_BUS.init(Mutex::new(spi));

        // W5500 pins
        let cs_w5500 = Output::new(p.PA4, Level::High, Speed::VeryHigh);
        let int_w5500 = ExtiInput::new(p.PB0, p.EXTI0, Pull::Up, Irqs);
        let reset_w5500 = Output::new(p.PB1, Level::High, Speed::VeryHigh);

        // SD CARD CS (boot phase only)
        let cs_sd = Output::new(p.PB12, Level::High, Speed::VeryHigh);

        // Gpio
        let button = ExtiInput::new(p.PB2, p.EXTI2, Pull::Up, Irqs);
        let bell = Output::new(p.PB3, Level::High, Speed::Low);
        let led = Output::new(p.PC13, Level::High, Speed::Low);

        // Can
        let mut can_bus = CanConfigurator::new(p.FDCAN1, p.PA11, p.PA12, Irqs);
        can_bus.set_bitrate(CAN_BITRATE);
        // let mut can_bus = can.into_internal_loopback_mode();
        let can_bus = can_bus.into_normal_mode();

        // Misc
        let rng = Rng::new(p.RNG, Irqs);
        let rtc = Rtc::new(p.RTC, RtcConfig::default());

        Self {
            buf_usart,
            can_bus,
            spi_bus,
            cs_w5500,
            int_w5500,
            reset_w5500,
            cs_sd,
            i2c_bus,
            bell,
            button,
            led,
            rng,
            rtc: rtc,
        }
    }
}
