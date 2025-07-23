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

#[cfg(test)]
mod tests {
    use std::io;

    use super::{invalid_data, padding_len, read_padding, write_padding};

    #[test]
    fn test_padding_len() {
        assert_eq!(padding_len(0), 0);
        assert_eq!(padding_len(1), 3);
        assert_eq!(padding_len(2), 2);
        assert_eq!(padding_len(3), 1);
        assert_eq!(padding_len(4), 0);
        assert_eq!(padding_len(5), 3);
        assert_eq!(padding_len(6), 2);
        assert_eq!(padding_len(7), 1);
        assert_eq!(padding_len(8), 0);
    }

    #[test]
    fn test_read_padding() {
        let mut buffer = [0; 4];

        // expected padding = 3
        let mut reader = io::Cursor::new(&mut buffer[..3]);
        read_padding(5, &mut reader).unwrap();
        assert_eq!(reader.position(), 3);

        // no padding needed
        let mut reader_edge = io::Cursor::new(&mut [0; 0]);
        read_padding(4, &mut reader_edge).unwrap();
        assert_eq!(reader_edge.position(), 0);
    }

    #[test]
    fn test_write_padding() {
        let mut buffer: [u8; 4] = [0; 4];

        // expected padding = 3
        let mut writer: io::Cursor<&mut [u8]> = io::Cursor::new(&mut buffer);
        write_padding(5, &mut writer).unwrap();
        assert_eq!(writer.position(), 3);

        // no padding needed
        let mut writer_edge: io::Cursor<&mut [u8]> = io::Cursor::new(&mut [0; 0]);
        write_padding(4, &mut writer_edge).unwrap();
        assert_eq!(writer_edge.position(), 0);
    }

    #[test]
    fn test_invalid_data() {
        let error = invalid_data("Test error message");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(format!("{}", error), "Test error message");
    }
}
