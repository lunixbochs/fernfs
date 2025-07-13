use std::io::{Read, Write};

pub const ALIGNMENT: usize = 4;

fn padding_len(src_len: usize) -> usize {
    (ALIGNMENT - (src_len % ALIGNMENT)) % ALIGNMENT
}

pub fn read_padding(src_len: usize, src: &mut impl Read) -> std::io::Result<()> {
    let pad_len = padding_len(src_len);
    if pad_len > 0 {
        let mut padding_buffer: [u8; ALIGNMENT] = Default::default();
        src.read_exact(&mut padding_buffer[..pad_len])?;
    }
    Ok(())
}

pub fn write_padding(src_len: usize, dest: &mut impl Write) -> std::io::Result<()> {
    let pad_len = padding_len(src_len);
    if pad_len > 0 {
        let padding_buffer: [u8; ALIGNMENT] = Default::default();
        dest.write_all(&padding_buffer[..pad_len])?;
    }
    Ok(())
}

pub fn invalid_data(m: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, m)
}
