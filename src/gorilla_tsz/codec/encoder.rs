
use std::mem;

use super::CodecMetadata;
use super::Measurement;
use super::double_to_int;
use super::super::utils::bitcopy;
use super::super::utils::bitcopy::BitValue;
use super::super::utils::varint;

pub enum EncoderError {
    Generic,
    BitCopyError(bitcopy::BitCopyError)
}

impl From<bitcopy::BitCopyError> for EncoderError {
    fn from(e: bitcopy::BitCopyError) -> Self {
        EncoderError::BitCopyError(e)
    }
}

fn to_bytes(val: u64) -> [u8; 8] {
    val.to_be_bytes()
}

fn write_bits(buf: &mut [u8], metadata: &mut CodecMetadata, src: &[u8], src_offbits: usize, nbits: usize) -> Result<(), EncoderError>
{
    bitcopy::copy(buf, src, nbits, metadata.buf_offbits, src_offbits)?;

    metadata.buf_offbits += nbits;
    Ok(())
}

fn write_varint(buf: &mut [u8], metadata: &mut CodecMetadata, value: u64) -> Result<(), EncoderError>
{
    let mut varint_buf = [0u8; 10];

    let sz = varint::encode(value, &mut varint_buf).unwrap();

    write_bits(buf, metadata, &varint_buf, 0, sz * 8)
}

fn write_double(buf: &mut [u8], metadata: &mut CodecMetadata, value: f64) -> Result<(), EncoderError>
{
    let int_val = unsafe { double_to_int(value) };
    let bytes = to_bytes(int_val);
    let nbits = mem::size_of::<f64>() * 8;

    write_bits(buf, metadata, &bytes, 0, nbits)
}

fn write_bit(buf: &mut [u8], metadata: &mut CodecMetadata, value: BitValue) -> Result<(), EncoderError>
{
    bitcopy::write_bit(buf, metadata.buf_offbits, value)?;
    metadata.buf_offbits += 1;
    Ok(())
}


pub fn encode(buf: &mut [u8], metadata: &mut CodecMetadata, measurement: &Measurement) -> Result<(), EncoderError>
{
    // Write first
    if metadata.idx == 0 {
        write_varint(buf, metadata, measurement.timestamp)?;
        write_varint(buf, metadata, measurement.count)?;
        write_double(buf, metadata, measurement.value)?;

        metadata.last_timestamp_delta = 0;
    } else {
        let last_measurement = match metadata.last_measurement {
            None => return Err(EncoderError::Generic),
            Some(measure) => measure
        };

        let timestamp_delta = measurement.timestamp as i64 - last_measurement.timestamp as i64;

        if metadata.idx == 1 {
            write_varint(buf, metadata, varint::encode_zigzag(timestamp_delta))?;
        } else {
            if timestamp_delta == metadata.last_timestamp_delta {
                write_bit(buf, metadata, BitValue::Zero)?;
            } else {
                write_bit(buf, metadata, BitValue::One)?;
                write_varint(buf, metadata, varint::encode_zigzag(timestamp_delta))?;
            }
        }

        let count_delta = measurement.count as i64 - last_measurement.count as i64;

        if count_delta == 0 {
            write_bit(buf, metadata, BitValue::Zero)?;
        } else {
            write_bit(buf, metadata, BitValue::One)?;

            // TODO: look at alternatives to varint encoding that assume small count deltas
            // ...the FB Gorilla paper did an analysis of their data to reduce here
            write_varint(buf, metadata, varint::encode_zigzag(count_delta))?;
        }

        let value_a = unsafe { double_to_int(last_measurement.value) };
        let value_b = unsafe { double_to_int(measurement.value) };

        let xor = value_a ^ value_b;

        if xor == 0 {
            write_bit(buf, metadata, BitValue::Zero)?;
        } else {
            write_bit(buf, metadata, BitValue::One)?;

            let curr_zeros = (xor.leading_zeros(), xor.trailing_zeros());

            println!("xor zeros: {:?} ({})\t-\t{:64b}", curr_zeros, (curr_zeros.0 + curr_zeros.1), xor);

            let mut written = false;
            if let Some(prev_xor) = metadata.value_xor {
                let prev_zeros = (prev_xor.leading_zeros(), prev_xor.trailing_zeros());

                if curr_zeros.0 >= prev_zeros.0 && curr_zeros.1 >= prev_zeros.1 {
                    println!("Using previous zeros of {:?} ({})", prev_zeros, (prev_zeros.0 + prev_zeros.1));

                    write_bit(buf, metadata, BitValue::Zero)?;

                    let fbytes = to_bytes(xor);
                    let bits = 64 - (prev_zeros.0 + prev_zeros.1);

                    write_bits(buf, metadata, &fbytes, prev_zeros.0 as usize, bits as usize)?;
                    written = true;
                }
            }

            if !written {
                println!("Not encoded..writing new xor with zeros: {:?}", curr_zeros);

                write_bit(buf, metadata, BitValue::One)?;
                let lead_zero_bits = [curr_zeros.0 as u8];
                let sig_bits = [64 - (curr_zeros.0 + curr_zeros.1) as u8];

                write_bits(buf, metadata, &lead_zero_bits, 8 - 6, 6)?;
                write_bits(buf, metadata, &sig_bits, 8 - 6, 6)?;

                let fbytes = to_bytes(xor);
                write_bits(buf, metadata, &fbytes, curr_zeros.0 as usize, sig_bits[0] as usize)?;
                metadata.value_xor = Some(xor);
            }
        }

        metadata.last_timestamp_delta = timestamp_delta;

    }

    metadata.last_measurement = Some(*measurement);
    metadata.idx += 1;

    Ok(())
}