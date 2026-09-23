const KEYWORD_LEN: usize = 6;

pub enum LinkyFrameError {
    ChecksumError,
}

enum ReceptionState {
    KeywordRead,
    DataRead,
    ChecksumRead,
}

#[derive(PartialEq)]
enum Frame {
    East,
    Sinsts,
}

pub struct Linky {
    east: Option<u32>,
    sinsts: Option<u32>,
    checksum: u8,
    data: u32,
    data_size: usize,
    frame: Option<Frame>,
    keyword_buf: [u8; 6],
    keyword_index: usize,
    state: ReceptionState,
}


impl Linky {
    pub fn new() -> Self {
        Linky {
            checksum: 0u8,
            data: 0,
            data_size: 0,
            east: None,
            frame: None,
            keyword_buf: [0; 6],
            keyword_index: 0,
            sinsts: None,
            state: ReceptionState::KeywordRead,
        }
    }

    pub fn decode_frame(&mut self, buf: &[u8], n: usize) {
        for c in &buf[..n] {
            match self.state {
                ReceptionState::KeywordRead => {
                    self.checksum = self.checksum.wrapping_add(*c);
                    if c.is_ascii_uppercase() {
                        if self.keyword_index < KEYWORD_LEN {
                            self.keyword_buf[self.keyword_index] = *c;
                            self.keyword_index += 1;
                        }
                    } else if *c == b'\t' {
                        if self.keyword_index == 4 || self.keyword_index == 6 {
                            if &self.keyword_buf[..4] == b"EAST" {
                                self.frame = Some(Frame::East);
                                self.state = ReceptionState::DataRead;
                                self.data = 0;
                                self.data_size = 0;
                            } else if &self.keyword_buf[..6] == b"SINSTS" {
                                self.frame = Some(Frame::Sinsts);
                                self.state = ReceptionState::DataRead;
                                self.data = 0;
                                self.data_size = 0;
                            } else {
                                self.checksum = 0;
                            }
                        } else {
                            self.checksum = 0;
                        }
                        self.keyword_index = 0;
                    } else {
                        self.checksum = 0;
                        self.keyword_index = 0;
                    }
                },
                ReceptionState::DataRead => {
                    self.checksum = self.checksum.wrapping_add(*c);
                    if *c == b'\t' {
                        if let Some(ref frame) = self.frame {
                            if (*frame == Frame::East && self.data_size == 9) ||
                               (*frame == Frame::Sinsts && self.data_size == 5) {
                                self.state = ReceptionState::ChecksumRead;
                            } else {
                                self.state = ReceptionState::KeywordRead;
                            }
                        }
                    } else if c.is_ascii_digit() {
                        if self.data_size < 9 {
                            self.data *= 10;
                            self.data += (*c as char).to_digit(10).unwrap();
                        }
                        self.data_size += 1;
                    } else {
                        self.state = ReceptionState::KeywordRead;
                    }
                },
                ReceptionState::ChecksumRead => {
                    if !c.is_ascii_control() {
                        if *c == (self.checksum & 0x3f) + b' ' {
                            // checksum is ok
                            if let Some(ref frame) = self.frame {
                                if *frame == Frame::East {
                                    self.east = Some(self.data);
                                } else {
                                    self.sinsts = Some(self.data);
                                }
                            }
                        }
                    }
                    self.state = ReceptionState::KeywordRead;
                }
            }
        }
    }

    pub fn get_east(&mut self) -> Option<u32> {
        let east = self.east;
        self.east = None;
        east
    }

    pub fn get_sinsts(&mut self) -> Option<u32> {
        let sinsts = self.sinsts;
        self.sinsts = None;
        sinsts
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_keyword() {
        let mut linky = Linky::new();
        let buf = b"\x02EASFT\t002050290\t!\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.get_east(), None);
    }

    #[test]
    fn bad_checksum() {
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t002050290\t#\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.get_east(), None);
    }

    #[test]
    fn bad_value_data() {
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t002G50290\t#\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.get_east(), None);
    }

    #[test]
    fn bad_length_data() {
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t00250290\t#\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.get_east(), None);
    }

    #[test]
    fn recovery_east_1() {
        // the fist keyword is bad
        let mut linky = Linky::new();
        let buf = b"\x02EASFT\t002050290\t!\n\x02EAST\t009999999\tN\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.get_east(), Some(9999999));
    }

    #[test]
    fn recovery_east_2() {
        // the first data length is bad
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t0020502900\t!\n\x02EAST\t009999999\tN\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.get_east(), Some(9999999));
    }

    #[test]
    fn recovery_east_3() {
        // the first data value is bad
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t00205F2900\t!\n\x02EAST\t009999999\tN\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.get_east(), Some(9999999));
    }

    #[test]
    fn nominal_east() {
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t002050290\t!\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.get_east(), Some(2050290));
        assert_eq!(linky.get_sinsts(), None);
    }

    #[test]
    fn nominal_chunked_east() {
        let mut linky = Linky::new();

        let buf = b"\x02EAST\t002";
        linky.decode_frame(buf, buf.len());

        let buf = b"050290\t!\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.get_east(), Some(2050290));
        assert_eq!(linky.get_sinsts(), None);
    }

    #[test]
    fn nominal_sinsts() {
        let mut linky = Linky::new();
        let buf = b"\x02SINSTS\t03521\tQ\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.get_east(), None);
        assert_eq!(linky.get_sinsts(), Some(3521));
    }

    #[test]
    fn nominal_mixed() {
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t002050290\t!\n\x02SINSTS\t03521\tQ\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.get_east(), Some(2050290));
        assert_eq!(linky.get_sinsts(), Some(3521));
    }

    #[test]
    fn value_max() {
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t999999999\t \n\x02SINSTS\t99999\t3\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.get_east(), Some(999999999));
        assert_eq!(linky.get_sinsts(), Some(99999));
    }

    #[test]
    fn real_case() {
        let mut linky = Linky::new();

        let buf = b"\nEAST	023591827	4\r";
        linky.decode_frame(buf, buf.len());

        let buf = b"\nSINSTS	00279	X\r";
        linky.decode_frame(buf, buf.len());

        assert_eq!(linky.get_east(), Some(23591827));
        assert_eq!(linky.get_sinsts(), Some(279));
    }
}
