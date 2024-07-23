#![no_std]
#![no_main]

mod api;
mod pio_ws2812;

use {
    core::iter::repeat,
    defmt::*,
    defmt_rtt as _,
    embassy_executor::Spawner,
    embassy_rp::{
        bind_interrupts,
        gpio::{Input, Level, Output, Pull},
        peripherals::{DMA_CH0, DMA_CH1, PIN_11, PIN_3, PIN_5, PIO0, PIO1, UART0},
        pio::{self, Pio},
        uart::{self, Async, BufferedUart},
    },
    embassy_time::{Duration, Ticker, Timer},
    panic_probe as _,
    pio_ws2812::Ws2812,
    smart_leds::{brightness, gamma, RGB8},
    static_cell::{ConstStaticCell, StaticCell},
};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    PIO1_IRQ_0 => pio::InterruptHandler<PIO1>;
    UART0_IRQ => uart::BufferedInterruptHandler<UART0>;
});

static ESP_UART_IN_BUF: ConstStaticCell<[u8; 1024]> = ConstStaticCell::new([0u8; 1024]);
static ESP_UART_OUT_BUF: ConstStaticCell<[u8; 1024]> = ConstStaticCell::new([0u8; 1024]);
static ESP_UART: StaticCell<BufferedUart<'static, UART0>> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Start");
    let p = embassy_rp::init(Default::default());

    spawner
        .spawn(heartbeat(p.PIO0, p.DMA_CH0, p.PIN_5))
        .unwrap();
    spawner.spawn(watch_usr_key(p.PIN_11, p.PIN_3)).unwrap();

    let mut config = uart::Config::default();
    config.baudrate = 115200;
    let uart = ESP_UART.init(BufferedUart::new(
        p.UART0,
        Irqs,
        p.PIN_0,
        p.PIN_1,
        ESP_UART_IN_BUF.take(),
        ESP_UART_OUT_BUF.take(),
        config,
    ));

    info!("Spawn server task");
    spawner.spawn(api::serve(spawner, uart)).unwrap();
}

#[embassy_executor::task]
async fn watch_usr_key(p: PIN_11, power_on: PIN_3) {
    let mut pin = Input::new(p, Pull::Up);
    let mut toggle = Level::Low;
    let mut power_on = Output::new(power_on, toggle);

    loop {
        pin.wait_for_falling_edge().await;
        info!("USR key pressed");
        toggle = if toggle == Level::High {
            Level::Low
        } else {
            Level::High
        };
        power_on.set_level(toggle);

        Timer::after(Duration::from_millis(1500)).await;
    }
}

#[embassy_executor::task]
async fn heartbeat(pio: PIO0, dma: DMA_CH0, pin: PIN_5) {
    const NUM_LEDS: usize = 144;
    let mut data = [RGB8::default(); NUM_LEDS];

    let Pio {
        mut common, sm0, ..
    } = Pio::new(pio, Irqs);
    let mut ws2812 = Ws2812::new(&mut common, sm0, dma, pin);

    // Loop forever making RGB values and pushing them out to the WS2812.
    let mut ticker = Ticker::every(Duration::from_millis(10));
    loop {
        for j in (0..170).chain((0..170).rev()) {
            gamma(brightness(
                repeat(RGB8::new(200, 130, 50)).take(NUM_LEDS),
                j + 30,
            ))
            .enumerate()
            .for_each(|(i, d)| data[i] = d);
            ws2812.write(&data).await;

            ticker.next().await;
        }
    }
}
