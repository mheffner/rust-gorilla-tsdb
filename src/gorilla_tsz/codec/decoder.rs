use super::CodecMetadata;
use super::Measurement;
use super::{int_to_double, double_to_int};
use super::super::utils::bitcopy;
use super::super::utils::bitcopy::BitValue;
use super::super::utils::varint;

#[derive(Debug)]
pub enum DecoderError {
    Generic(String),
    BitCopyError(bitcopy::BitCopyError),
    VarintError(varint::VarIntError)
}

impl From<bitcopy::BitCopyError> for DecoderError {
    fn from(e: bitcopy::BitCopyError) -> Self {
        DecoderError::BitCopyError(e)
    }
}

impl From<varint::VarIntError> for DecoderError {
    fn from(e: varint::VarIntError) -> Self {
        DecoderError::VarintError(e)
    }
}

fn delta_add(val: u64, delta: i64) -> u64
{
    (val as i64 + delta) as u64
}

fn read_varint(buf: &[u8], metadata: &mut CodecMetadata) -> Result<u64, DecoderError>
{
    let mut bytes = [0u8; 10];

    for i in 0..bytes.len() {
        bitcopy::copy(&mut bytes[i..], buf, 8, 0, metadata.buf_offbits + i * 8)?;

        //
        // XXX: huge abstraction leak here, varint::decode should take a stream and read a byte at a time
        //
        if bytes[i] < 128 {
            let (value, sz) = varint::decode(&bytes)?;

            metadata.buf_offbits += sz * 8;
            return Ok(value);
        }
    }

    Err(DecoderError::Generic("Could not find end of varint".to_string()))
}

fn read_double(buf: &[u8], metadata: &mut CodecMetadata) -> Result<f64, DecoderError>
{
    let mut bytes = [0u8; 8];

    bitcopy::copy(&mut bytes, buf, 8 * 8, 0, metadata.buf_offbits)?;
    metadata.buf_offbits += 8 * 8;

    let intval = u64::from_be_bytes(bytes);

    Ok(unsafe { int_to_double(intval) })
}

fn read_bit(buf: &[u8], metadata: &mut CodecMetadata) -> Result<BitValue, DecoderError>
{

    let result = bitcopy::read_bit(buf, metadata.buf_offbits)?;
    metadata.buf_offbits += 1;

    Ok(result)
}

fn read_bits(src: &[u8], metadata: &mut CodecMetadata, dst: &mut [u8], dst_offbits: usize, nbits: usize) -> Result<(), DecoderError>
{

    bitcopy::copy(dst, src, nbits, dst_offbits, metadata.buf_offbits)?;
    metadata.buf_offbits += nbits;

    Ok(())
}

pub fn decode(buf: &[u8], metadata: &mut CodecMetadata) -> Result<Measurement, DecoderError>
{
    let measurement;

    if metadata.idx == 0 {
        let timestamp = read_varint(buf, metadata)?;
        let count = read_varint(buf, metadata)?;
        let value = read_double(buf, metadata)?;

        measurement = Measurement{timestamp, count, value};
    } else {
        let last_measurement = match metadata.last_measurement {
            None => return Err(DecoderError::Generic("No previous measurement".to_string())),
            Some(measure) => measure
        };

        let timestamp;

        if metadata.idx == 1 {
            let timestamp_delta = varint::decode_zigzag(read_varint(buf, metadata)?);
            timestamp = delta_add(last_measurement.timestamp, timestamp_delta);
            metadata.last_timestamp_delta = timestamp_delta;
        } else {
            match read_bit(buf, metadata)? {
                BitValue::Zero => {
                    timestamp = delta_add(last_measurement.timestamp, metadata.last_timestamp_delta);
                },
                BitValue::One => {
                    let timestamp_delta = varint::decode_zigzag(read_varint(buf, metadata)?);
                    timestamp = delta_add(last_measurement.timestamp, timestamp_delta);
                    metadata.last_timestamp_delta = timestamp_delta;
                }
            }
        }

        let count;
        match read_bit(buf, metadata)? {
            BitValue::Zero => {
                count = last_measurement.count;
            },
            BitValue::One => {
                let count_delta = varint::decode_zigzag(read_varint(buf, metadata)?);
                count = delta_add(last_measurement.count, count_delta);
            }
        }

        let value;
        match read_bit(buf, metadata)? {
            BitValue::Zero => {
                value = last_measurement.value
            },
            BitValue::One => {
                match read_bit(buf, metadata)? {
                    BitValue::Zero => {
                        let xor = match metadata.value_xor {
                            // Should never happen
                            None => return Err(DecoderError::Generic("No previous xor value".to_string())),
                            Some(xor) => xor
                        };

                        let zeros = (xor.leading_zeros(), xor.trailing_zeros());
                        let mut bytes = [0u8; 8];

                        read_bits(buf, metadata, &mut bytes, zeros.0 as usize, 64 - (zeros.0 + zeros.1) as usize)?;

                        let prev_value = unsafe { double_to_int(last_measurement.value) };
                        let curr_value = prev_value ^ u64::from_be_bytes(bytes);

                        value = unsafe { int_to_double(curr_value) };
                    },
                    BitValue::One => {
                        let mut leading_zeros = [0u8; 1];
                        let mut sig_bits = [0u8; 1];

                        read_bits(buf, metadata, &mut leading_zeros, 8 - 6, 6)?;
                        read_bits(buf, metadata, &mut sig_bits, 8 - 6, 6)?;

                        let mut bytes = [0u8; 8];
                        read_bits(buf, metadata, &mut bytes, leading_zeros[0] as usize, sig_bits[0] as usize)?;

                        let xor = u64::from_be_bytes(bytes);

                        let prev_value = unsafe { double_to_int(last_measurement.value) };
                        let curr_value = prev_value ^ xor;

                        value = unsafe { int_to_double(curr_value) };
                        metadata.value_xor = Some(xor);
                    }
                }
            }
        }

        measurement = Measurement{timestamp, count, value};
    }

    metadata.last_measurement = Some(measurement);
    metadata.idx += 1;

    Ok(measurement)
}