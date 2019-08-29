use std::mem;
use std::cmp::min;

#[derive(Debug)]
pub struct BitCopyError;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BitValue {
    Zero,
    One
}

fn bitmask_lower(bits: usize) -> Result<u8, BitCopyError>
{
    let wordsizebits = mem::size_of::<u8>() * 8;

    if bits > wordsizebits {
        return Err(BitCopyError);
    }

    if bits == wordsizebits {
        Ok(0xFF)
    } else {
        Ok((0x01u8 << bits) - 1)
    }
}

pub fn write_bit(dst_buf: &mut [u8], dst_offbits: usize, bit: BitValue) -> Result<(), BitCopyError> {
    let bit_byte = match bit {
        BitValue::One => [0x01u8],
        BitValue::Zero => [0x00u8]
    };

    copy(dst_buf, &bit_byte, 1, dst_offbits, (mem::size_of::<u8>() * 8) - 1)
}

pub fn read_bit(src_buf: &[u8], src_offbits: usize) -> Result<BitValue, BitCopyError> {
    let mut bit_byte = [0u8];

    match copy(&mut bit_byte, src_buf, 1, (mem::size_of::<u8>() * 8) - 1, src_offbits) {
        Err(e) => Err(e),
        Ok(()) => {
            match bit_byte[0] {
                0x01 => Ok(BitValue::One),
                _ => Ok(BitValue::Zero)
            }
        }
    }
}

// TODO: make generic, ie. would be more efficient with u64 word copies
pub fn copy(dst_buf: &mut [u8], src_buf: &[u8], nbits: usize,
            dst_offbits: usize, src_offbits: usize) -> Result<(), BitCopyError>
{

    let mut dst_offbits = dst_offbits;
    let mut src_offbits = src_offbits;
    let mut nbits = nbits;

    let copysize: usize = mem::size_of::<u8>();
    let copybitsize: usize = copysize * 8;

    // Steps:
    //
    // 1. determine how many bits we can max copy from src, based on its offset
    //    copy into temp
    // 2. determine how many bits we can max copy into dst, based on its offset
    //    leave some in temp var if not copied?
    //
    // 3. if remaining bits in temp (ie src copied > dst copied), shift dst and copy into it
    //
    //  bump dst and src offsets and repeat?

    while nbits > 0 {
        let src_bit_offset = src_offbits & (copybitsize - 1);
        let src_bits = copybitsize - src_bit_offset;
        let bits_to_copy = min(nbits, src_bits);

        let src_idx = src_offbits / copybitsize;

        if src_idx >= src_buf.len() {
            return Err(BitCopyError{});
        }

        // We may mask more bits than we need
        let src_mask = bitmask_lower(src_bits)?;

        // Mask out bits and shift to only keep those we're copying
        let byte = (src_buf[src_idx] & src_mask) >> (src_bits - bits_to_copy);

        // We are done with source now
        src_offbits += bits_to_copy;

        let mut dst_copy_bits = bits_to_copy;

        let dst_bit_offset = dst_offbits & (copybitsize - 1);
        let dst_bits = copybitsize - dst_bit_offset;

        let dst_idx = dst_offbits / copybitsize;

        if dst_idx >= dst_buf.len() {
            return Err(BitCopyError{});
        }

        let byte_mask = bitmask_lower(dst_copy_bits)?;

        if dst_copy_bits <= dst_bits {
            if dst_idx >= dst_buf.len() {
                return Err(BitCopyError{});
            }

            dst_buf[dst_idx] = (dst_buf[dst_idx] & !bitmask_lower(dst_bits)?) |
                ((byte & byte_mask) << (dst_bits - dst_copy_bits));
        } else {
            // We'll copy two words here
            if (dst_idx + 1) >= dst_buf.len() {
                return Err(BitCopyError{});
            }

            dst_buf[dst_idx] = (dst_buf[dst_idx] & !bitmask_lower(dst_bits)?) |
                ((byte & byte_mask) >> (dst_copy_bits - dst_bits));

            // Move the remaining bits into the top of the next word
            dst_copy_bits -= dst_bits;
            dst_buf[dst_idx + 1] = (byte & byte_mask) << (copybitsize - dst_copy_bits);
        }

        dst_offbits += bits_to_copy;
        nbits -= bits_to_copy;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::varint::{encode, decode};

    #[test]
    fn test_simple_offset_bitcopies() {
        let mut in_byte = [0u8; 1];
        let mut out_byte = [0u8; 1];
        let mut buf = [0u8; 2];
        let offset_bits = 3;
        let mut dst_offbits = 0;
        let mut src_offbits = 0;

        in_byte[0] = 18;

        for i in 0..offset_bits {
            assert_eq!(write_bit(&mut buf, dst_offbits, BitValue::One).ok(), Some(()));
            dst_offbits += 1;
        }
        assert_eq!(copy(&mut buf, &in_byte, 6, dst_offbits, 8 - 6).ok(), Some(()));

        for i in 0..offset_bits {
            assert_eq!(read_bit(&buf, src_offbits).ok(), Some(BitValue::One));
            src_offbits += 1;
        }
        assert_eq!(copy(&mut out_byte, &buf, 6, 8 - 6, src_offbits).ok(), Some(()));

        assert_eq!(out_byte[0], in_byte[0]);
    }

    #[test]
    fn test_bit_copy_bitpattern() {
        const COUNT: usize = 10;
        let mut buf = [0u8; COUNT];

        let mut dst_off = 0;
        for i in 0..(COUNT * 8) {
            let mut b = [0u8];

            if (i % 2) != 0 {
                b[0] |= 1 << 7;
            }

            let r = copy(&mut buf, &b, 1, dst_off, 0);
            assert_eq!(r.ok(), Some(()));

            dst_off += 1;
        }

        let mut src_off = 0;
        for i in 0..(COUNT * 8) {
            let mut b = [0u8];

            let r = copy(&mut b, &buf, 1, 0, src_off);
            assert_eq!(r.ok(), Some(()));

            if (i % 2) != 0 {
                assert_eq!(b[0], 1 << 7);
            } else {
                assert_eq!(b[0], 0u8);
            }

            src_off += 1;
        }
    }

    #[test]
    fn test_bit_copy() {
        verify_varint_copy(&[0, 1, 2048, 16384, 1 << 16, 1 << 24, 1 << 32, 1 << 48, 1 << 63], None);
        verify_varint_copy(&[0, 1, 2048, 16384, 1 << 16, 1 << 24, 1 << 32, 1 << 48, 1 << 63], Some(1));
        verify_varint_copy(&[0, 1, 2048, 16384, 1 << 16, 1 << 24, 1 << 32, 1 << 48, 1 << 63], Some(2));
        verify_varint_copy(&[0, 1, 2048, 16384, 1 << 16, 1 << 24, 1 << 32, 1 << 48, 1 << 63], Some(3));
        verify_varint_copy(&[0, 1, 2048, 16384, 1 << 16, 1 << 24, 1 << 32, 1 << 48, 1 << 63], Some(4));
        verify_varint_copy(&[0, 1, 2048, 16384, 1 << 16, 1 << 24, 1 << 32, 1 << 48, 1 << 63], Some(5));
        verify_varint_copy(&[0, 1, 2048, 16384, 1 << 16, 1 << 24, 1 << 32, 1 << 48, 1 << 63], Some(6));
        verify_varint_copy(&[0, 1, 2048, 16384, 1 << 16, 1 << 24, 1 << 32, 1 << 48, 1 << 63], Some(7));
    }

    fn verify_varint_copy(values: &[u64], weave_bits: Option<usize>) {
        let weavesize = match weave_bits {
            None => 0,
            Some(n) => (values.len() * n) / 8 + 1
        };
        let mut buf = vec![0; (values.len() * 10) + weavesize].into_boxed_slice();
        let mut int_buf: [u8; 10] = [0; 10];

        let mut dst_offbits = 0;
        let mut encoded = Vec::new();
        for value in values {
            let sz = encode(*value, &mut int_buf).unwrap();

            encoded.push((*value, sz));
            match copy(&mut buf, &int_buf, sz * 8, dst_offbits, 0) {
                Err(e) => assert!(false),
                Ok(()) => assert!(true)
            };

            dst_offbits += sz * 8;

            match weave_bits {
                Some(num) => {
                    for _ in 0..num {
                        assert_eq!(write_bit(&mut buf, dst_offbits, BitValue::One).ok(), Some(()));
                        dst_offbits += 1;
                    }
                },
                None => {}
            }
        }

        println!("decoding");
        let mut src_offbits = 0;
        for (value, sz) in encoded {

            match copy(&mut int_buf, &buf, sz * 8, 0, src_offbits) {
                Err(e) => assert!(false),
                Ok(()) => {
                    assert_eq!(decode(&int_buf).ok(), Some((value, sz)))
                }
            };

            src_offbits += sz * 8;

            match weave_bits {
                Some(num) => {
                    for _ in 0..num {
                        assert_eq!(read_bit(&buf, src_offbits).ok(), Some(BitValue::One));
                        src_offbits += 1;
                    }
                },
                None => {}
            }
        }
    }
}