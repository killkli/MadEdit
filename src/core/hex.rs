pub struct HexDump {
    pub offsets: String,
    pub hex: String,
    pub ascii: String,
}

pub fn generate_hex_dump(data: &[u8], max_bytes: usize) -> HexDump {
    let limit = data.len().min(max_bytes);
    let slice = &data[..limit];

    let mut offsets = String::new();
    let mut hex = String::new();
    let mut ascii = String::new();

    for (chunk_idx, chunk) in slice.chunks(16).enumerate() {
        let offset = chunk_idx * 16;
        offsets.push_str(&format!("{:08X}\n", offset));

        // Format hex bytes
        let mut hex_line = String::with_capacity(50);
        for (i, &b) in chunk.iter().enumerate() {
            if i == 8 {
                hex_line.push(' ');
            }
            hex_line.push_str(&format!("{:02X} ", b));
        }

        // Padding if last line has fewer than 16 bytes
        if chunk.len() < 16 {
            for i in chunk.len()..16 {
                if i == 8 {
                    hex_line.push(' ');
                }
                hex_line.push_str("   ");
            }
        }
        hex.push_str(hex_line.trim_end());
        hex.push('\n');

        // Format ASCII characters
        let mut ascii_line = String::with_capacity(16);
        for &b in chunk {
            if b >= 0x20 && b <= 0x7E {
                ascii_line.push(b as char);
            } else {
                ascii_line.push('.');
            }
        }
        ascii.push_str(&ascii_line);
        ascii.push('\n');
    }

    if data.len() > limit {
        offsets.push_str("...\n");
        hex.push_str(&format!("... (已截斷，共 {} 位元組)\n", data.len()));
        ascii.push_str("...\n");
    }

    HexDump {
        offsets,
        hex,
        ascii,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_hex_dump() {
        let data = b"Hello, World!";
        let dump = generate_hex_dump(data, 100);
        assert!(dump.offsets.starts_with("00000000"));
        assert!(dump.hex.contains("48 65 6C 6C 6F"));
        assert!(dump.ascii.contains("Hello, World!"));
    }
}
