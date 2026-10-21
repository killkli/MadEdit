/// 十六進位檢視區的三欄格式化文字結果（位址偏移、十六進位位元組、ASCII 可視字元）
pub struct HexDump {
    pub offsets: String,
    pub hex: String,
    pub ascii: String,
}

/// 將原始位元組序列格式化為每列 16 位元組的三欄對照 Hex Dump，最多預覽 max_bytes 位元組以避免 UI 卡頓
pub fn generate_hex_dump(data: &[u8], max_bytes: usize) -> HexDump {
    let limit = data.len().min(max_bytes);
    let slice = &data[..limit];

    let mut offsets = String::new();
    let mut hex = String::new();
    let mut ascii = String::new();

    for (chunk_idx, chunk) in slice.chunks(16).enumerate() {
        let offset = chunk_idx * 16;
        offsets.push_str(&format!("{:08X}\n", offset));

        // 格式化每列 16 個十六進位位元組，於第 8 個位元組處額外插入空白以利閱讀對齊
        let mut hex_line = String::with_capacity(50);
        for (i, &b) in chunk.iter().enumerate() {
            if i == 8 {
                hex_line.push(' ');
            }
            hex_line.push_str(&format!("{:02X} ", b));
        }

        // 若最末列不滿 16 位元組，補齊空白以維持版面寬度一致
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

        // 格式化可列印 ASCII 字元（0x20..=0x7E），控制字元與非 ASCII 位元組以 '.' 代表
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
