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

pub struct Linky {
    pub east: Option<u32>,
    pub sinst: Option<u32>,
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
            sinst: None,
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
                        }
                        self.keyword_index += 1;
                    } else if *c == b'\t' {
                        if self.keyword_index == 4 || self.keyword_index == 5 {
                            if &self.keyword_buf[..4] == b"EAST" {
                                self.frame = Some(Frame::East);
                                self.state = ReceptionState::DataRead;
                                self.data = 0;
                                self.data_size = 0;
                            } else if &self.keyword_buf[..5] == b"SINST" {
                                self.frame = Some(Frame::Sinst);
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
                               (*frame == Frame::Sinst && self.data_size == 5) {
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
                        if *c == (self.checksum + b' ') & 0x7f {
                            // checksum is ok
                            if let Some(ref frame) = self.frame {
                                if *frame == Frame::East {
                                    self.east = Some(self.data);
                                } else {
                                    self.sinst = Some(self.data);
                                }
                            }
                        }
                    }
                    self.state = ReceptionState::KeywordRead;
                }
            }
        }
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
        assert_eq!(linky.east, None);
    }

    #[test]
    fn bad_password() {
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t002050290\t#\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.east, None);
    }

    #[test]
    fn bad_value_data() {
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t002G50290\t#\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.east, None);
    }

    #[test]
    fn bad_length_data() {
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t00250290\t#\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.east, None);
    }

    #[test]
    fn recovery_east_1() {
        // the fist keyword is bad
        let mut linky = Linky::new();
        let buf = b"\x02EASFT\t002050290\t!\n\x02EAST\t009999999\tN\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.east, Some(9999999));
    }

    #[test]
    fn recovery_east_2() {
        // the first data length is bad
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t0020502900\t!\n\x02EAST\t009999999\tN\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.east, Some(9999999));
    }

    #[test]
    fn recovery_east_3() {
        // the first data value is bad
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t00205F2900\t!\n\x02EAST\t009999999\tN\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.east, Some(9999999));
    }

    #[test]
    fn nominal_east() {
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t002050290\t!\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.east, Some(2050290));
        assert_eq!(linky.sinst, None);
    }

    #[test]
    fn nominal_chunked_east() {
        let mut linky = Linky::new();

        let buf = b"\x02EAST\t002";
        linky.decode_frame(buf, buf.len());

        let buf = b"050290\t!\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.east, Some(2050290));
        assert_eq!(linky.sinst, None);
    }

    #[test]
    fn nominal_sinst() {
        let mut linky = Linky::new();
        let buf = b"\x02SINST\t03521\t>\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.east, None);
        assert_eq!(linky.sinst, Some(3521));
    }

    #[test]
    fn nominal_mixed() {
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t002050290\t!\n\x02SINST\t03521\t>\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.east, Some(2050290));
        assert_eq!(linky.sinst, Some(3521));
    }

    #[test]
    fn value_max() {
        let mut linky = Linky::new();
        let buf = b"\x02EAST\t999999999\t`\n\x02SINST\t99999\t`\n";
        linky.decode_frame(buf, buf.len());
        assert_eq!(linky.east, Some(999999999));
        assert_eq!(linky.sinst, Some(99999));
    }
}
