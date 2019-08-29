
#[derive(Debug)]
pub struct VarIntError;

pub fn encode(val: u64, buf: &mut [u8]) -> Result<usize, VarIntError> {
    let mut byte: u8;
    let mut idx: usize = 0;

    let mut val = val;
    while val >= 128 {
        if idx >= buf.len() {
            return Err(VarIntError{});
        }

        byte = (0x80 | (val & 0x7f)) as u8;
        buf[idx] = byte;
        val >>= 7;
        idx += 1;
    }

    if idx >= buf.len() {
        return Err(VarIntError{});
    }

    byte = val as u8;
    buf[idx] = byte;

    Ok(idx + 1)
}

pub fn decode(buf: &[u8]) -> Result<(u64, usize), VarIntError> {
    let mut idx: usize = 0;
    let mut val = 0u64;
    let mut shift = 0u64;

    loop {
        if idx >= buf.len() {
            return Err(VarIntError{})
        }
        let byte = buf[idx];
        val |= ((byte & 0x7f) as u64) << shift;
        if byte < 128 {
            break;
        }
        shift += 7;
        idx += 1;
    }

    Ok((val, idx + 1))
}

pub fn encode_zigzag(val: i64) -> u64 {
    let uval_l = val as u64;
    let uval_r = (val >> 63) as u64;

    return (uval_l << 1) ^ uval_r;
}

pub fn decode_zigzag(val: u64) -> i64 {
    let ival_l = (val >> 1) as i64;
    let ival_r = (val & 1) as i64;

    return ival_l ^ -(ival_r);
}

#[test]
fn test_varint_encoding() {
    fn _verify_varint(val: u64, sz: usize) {
        let mut buf = [0u8; 10];

        assert_eq!(encode(val, &mut buf).unwrap(), sz);
        assert_eq!(decode(&buf).ok(), Some((val, sz)));
    }

    _verify_varint(12345, 2);
    _verify_varint(0, 1);
    _verify_varint(!(0x0), 10);
    _verify_varint(50, 1);
    _verify_varint(-1i64 as u64, 10);

    let mut buf = [0u8; 10];

    assert_eq!(encode(1 as u64, &mut buf).unwrap(), 1);
    assert!(decode(&vec![128]).is_err());

    assert_eq!(encode_zigzag(-1), 1);
    assert_eq!(decode_zigzag(encode_zigzag(-5)), -5);

    _verify_varint(encode_zigzag(-1), 1);
}