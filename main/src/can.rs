use defmt::info;
use embassy_executor::task;
// use embassy_time::Timer;
use embassy_stm32::can::Can;

#[task]
pub async fn task(can: Can<'static>) {
    let (mut _tx, mut rx, _props) = can.split();

    loop {
        if let Ok(envelope) = rx.read().await {
            let (rx_frame, _ts) = envelope.parts();
            let data = rx_frame.data();

            let buf = [data[0], data[1]];
            let t: i16 = i16::from_be_bytes(buf).try_into().unwrap();
            let buf = [data[2], data[3]];
            let h: u16 = u16::from_be_bytes(buf).try_into().unwrap();

            info!("temperature: {}, humidity: {}", t, h);
        };
    };
}
