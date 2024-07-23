use core::{f32::consts::E, fmt::Pointer, str::from_utf8};

use defmt::{info, unwrap, warn};
use embassy_executor::Spawner;
use embassy_rp::{
    clocks::RoscRng,
    gpio::{Level, Output},
    peripherals::{DMA_CH0, DMA_CH1, PIN_23, PIN_24, PIN_25, PIN_29, PIO0, PIO1, UART0},
    pio::Pio,
    uart::Async,
    uart::BufferedUart,
    uart::Error,
};
use embassy_time::{with_timeout, Duration, TimeoutError, Timer};
use embedded_io_async::{Read, Write};

#[derive(Debug)]
enum CommandError {
    Timeout(TimeoutError),
    UartError(Error),
}

impl From<TimeoutError> for CommandError {
    fn from(e: TimeoutError) -> Self {
        CommandError::Timeout(e)
    }
}

impl From<Error> for CommandError {
    fn from(e: Error) -> Self {
        CommandError::UartError(e)
    }
}

#[embassy_executor::task]
pub(crate) async fn serve(spawner: Spawner, uart: &'static mut BufferedUart<'static, UART0>) {
    let wifi_network = env!("WIFI_NETWORK");
    let wifi_password = env!("WIFI_PASSWORD");

    let in_buf = &mut [0u8; 1024];

    info!("Connecting to WiFi");
    send_command(uart, b"ATE0").await.unwrap();

    send_command(uart, b"AT+CWMODE=1").await.unwrap();
    send_command(uart, b"AT+CIPMUX=1").await.unwrap();
    send_command(uart, b"AT+CIPSERVER=1").await.unwrap();

    info!("Done")
}

async fn send_command(
    uart: &mut BufferedUart<'static, UART0>,
    command: &[u8],
) -> Result<(), CommandError> {
    info!("Sending: {:?}", from_utf8(command).unwrap());
    let in_buf = &mut [0u8; 128];
    uart.write(command).await?;
    uart.write(b"\r\n").await?;

    let mut newlines = 0;
    let mut waits = 0;

    loop {
        let amount_read = with_timeout(Duration::from_millis(10), uart.read(in_buf)).await??;
        info!(
            "Received: {:?}",
            from_utf8(in_buf.get(..amount_read).unwrap()).unwrap()
        );

        in_buf.iter().take(amount_read).for_each(|&c| {
            if c == b'\n' {
                newlines += 1;
            }
        });

        if newlines >= 2 {
            break;
        }
    }

    return Ok(());
}
