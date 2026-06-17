use defmt::info;
use embassy_executor::task;
// use embassy_time::Timer;
use embassy_stm32::{
    bind_interrupts,
    Peri, peripherals::{PA9, PA10, USART1},
    usart::{self, BufferedUart, Config, DataBits, Parity, StopBits}
};
use embedded_io_async::Read;

bind_interrupts!(struct Irqs {
    USART1 => usart::BufferedInterruptHandler<USART1>;
});

const KEYWORD_LEN: usize = 6;

enum ReceptionState {
    KeywordRead,
    DataRead,
    ChecksumRead,
}

#[derive(PartialEq)]
enum Frame {
    East,
    Sinst,
}

struct Variables {
    east: Option<u32>,
    checksum: u8,
    data: u32,
    data_size: usize,
    frame: Option<Frame>,
    keyword_buf: [u8; 6],
    keyword_index: usize,
    sinst: Option<u32>,
    state: ReceptionState,
}


fn decode_frame(buf: &[u8], n: usize, vars: &mut Variables) {
    for c in &buf[..n] {
        match vars.state {
            ReceptionState::KeywordRead => {
                vars.checksum = vars.checksum.wrapping_add(*c);
                if c.is_ascii_uppercase() {
                    if vars.keyword_index < KEYWORD_LEN {
                        vars.keyword_buf[vars.keyword_index] = *c;
                        vars.keyword_index += 1;
                    }
                } else if *c == b' ' {
                    if vars.keyword_index == 4 || vars.keyword_index == 5 {
                        if &vars.keyword_buf[..4] == b"EAST" {
                            vars.frame = Some(Frame::East);
                            vars.state = ReceptionState::DataRead;
                            vars.data = 0;
                            vars.data_size = 0;
                        } else if &vars.keyword_buf[..5] == b"SINST" {
                            vars.frame = Some(Frame::Sinst);
                            vars.state = ReceptionState::DataRead;
                            vars.data = 0;
                            vars.data_size = 0;
                        } else {
                            vars.checksum = 0;
                        }
                    } else {
                        vars.checksum = 0;
                    }
                    vars.keyword_index = 0;
                } else {
                    vars.checksum = 0;
                    vars.keyword_index = 0;
                }
            },
            ReceptionState::DataRead => {
                vars.checksum = vars.checksum.wrapping_add(*c);
                if *c == b' ' {
                    if let Some(ref frame) = vars.frame {
                        if (*frame == Frame::East && vars.data_size == 5) || (*frame == Frame::Sinst && vars.data_size == 9) {
                            vars.state = ReceptionState::ChecksumRead;
                        } else {
                            vars.state = ReceptionState::KeywordRead;
                        }
                    }
                } else if c.is_ascii_digit() {
                    if vars.data_size < 9 {
                        vars.data *= 10;
                        vars.data += (*c as char).to_digit(10).unwrap();
                        vars.data_size += 1;
                    }
                } else {
                    vars.state = ReceptionState::KeywordRead;
                }
            },
            ReceptionState::ChecksumRead => {
                if !c.is_ascii_control() {
                    if *c == vars.checksum + b' ' {
                        // checksum is ok
                        if let Some(ref frame) = vars.frame {
                            if *frame == Frame::East {
                                vars.east = Some(vars.data);
                            } else {
                                vars.sinst = Some(vars.data);
                            }
                            vars.checksum = 0;
                            vars.data = 0;
                        }
                    } else {
                        vars.state = ReceptionState::KeywordRead;
                    }
                } else {
                    vars.state = ReceptionState::KeywordRead;
                }
            }
        }
    }
}


#[task]
pub async fn task(uart: Peri<'static, USART1>, tx: Peri<'static, PA9>, rx: Peri<'static, PA10>) {
    let mut config = Config::default();
    // set Linky serial line configuration
    config.baudrate = 9600;
    config.parity = Parity::ParityEven;
    config.data_bits = DataBits::DataBits7;
    config.stop_bits =  StopBits::STOP1;

    let mut tx_buf  = [0u8; 32];
    let mut rx_buf = [0u8; 32];
    let mut buf = [0u8; 32];
    let mut buf_usart = BufferedUart::new(uart, rx, tx, &mut tx_buf, &mut rx_buf, Irqs, config).unwrap();

    let mut vars = &mut Variables{
        checksum: 0u8,
        data: 0,
        data_size: 0,
        east: None,
        frame: None,
        keyword_buf: [0; 6],
        keyword_index: 0,
        sinst: None,
        state: ReceptionState::KeywordRead,
    };

    loop {
        let n = buf_usart.read(&mut buf).await.unwrap();
        info!("Received: {}", buf);

        decode_frame(&buf, n, &mut vars);

        // Timer::after_millis(500).await;
    };
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nominal_east() {
        let mut vars = &mut Variables{
            checksum: 0u8,
            data: 0,
            data_size: 0,
            east: None,
            frame: None,
            keyword_buf: [0; 6],
            keyword_index: 0,
            sinst: None,
            state: ReceptionState::KeywordRead,
        };
        let buf = b"EAST 002050290 !";
        decode_frame(buf, buf.len(), &mut vars);
    }
}
