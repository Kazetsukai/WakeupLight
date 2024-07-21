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
        peripherals::{DMA_CH0, PIN_11, PIN_3, PIN_5, PIO0, PIO1, UART0},
        pio::{InterruptHandler, Pio},
        uart::{self, Uart},
    },
    embassy_time::{Duration, Ticker, Timer},
    panic_probe as _,
    pio_ws2812::Ws2812,
    smart_leds::{brightness, gamma, RGB8},
};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    PIO1_IRQ_0 => InterruptHandler<PIO1>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Start");
    let p = embassy_rp::init(Default::default());

    spawner
        .spawn(heartbeat(p.PIO0, p.DMA_CH0, p.PIN_5))
        .unwrap();
    spawner.spawn(watch_usr_key(p.PIN_11, p.PIN_3)).unwrap();
    /*spawner
    .spawn(api::serve(
        spawner, p.PIO1, p.DMA_CH1, p.PIN_23, p.PIN_25, p.PIN_24, p.PIN_29,
    ))
    .unwrap();*/

    Input::new(p.PIN_0, Pull::None);
    Input::new(p.PIN_1, Pull::None);
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
        for j in (0..255).chain((0..255).rev()) {
            gamma(brightness(
                repeat(RGB8::new(200, 130, 50)).take(NUM_LEDS),
                j,
            ))
            .enumerate()
            .for_each(|(i, d)| data[i] = d);
            ws2812.write(&data).await;

            ticker.next().await;
        }
    }
}
