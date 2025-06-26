use std::io::{Read, Write};

pub const ALIGMENT: usize = 4;

pub fn read_padding(src_len: usize, src: &mut impl Read) -> std::io::Result<()> {
    let mut padding_buffer: [u8; ALIGMENT] = Default::default();
    src.read_exact(&mut padding_buffer[(src_len % ALIGMENT)..])
}

pub fn write_padding(src_len: usize, dest: &mut impl Write) -> std::io::Result<()> {
    let padding_buffer: [u8; ALIGMENT] = Default::default();
    dest.write_all(&padding_buffer[(src_len % ALIGMENT)..])
}

pub fn invalid_data(m: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, m)
}
