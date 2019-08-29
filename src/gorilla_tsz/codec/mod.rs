use std::mem;

pub mod encoder;
pub mod decoder;

use super::Measurement;

pub struct CodecMetadata {
    idx: i32,
    buf_offbits: usize,

    last_timestamp_delta: i64,
    last_measurement: Option<Measurement>,
    value_xor: Option<u64>
}

impl CodecMetadata {
    pub fn new() -> CodecMetadata {
        CodecMetadata {idx: 0, buf_offbits: 0, last_timestamp_delta: 0, last_measurement: None, value_xor: None }
    }

    // Rounds up
    pub fn byte_len(self: &CodecMetadata) -> usize {
        return (self.buf_offbits + 7) / 8;
    }
}


unsafe fn double_to_int(value: f64) -> u64
{
    mem::transmute::<f64, u64>(value)
}

unsafe fn int_to_double(val: u64) -> f64
{
    mem::transmute::<u64, f64>(val)
}


#[cfg(test)]
mod tests {
    use super::Measurement;
    use super::encoder::encode;
    use super::decoder::decode;
    use super::CodecMetadata;

    fn measure_is_close(a: &Measurement, b: &Measurement) -> bool {
        const THRESH: f64 = 0.000001;

        a.timestamp == b.timestamp &&
            a.count == b.count &&
            f64::abs(a.value - b.value) < THRESH
    }

    #[test]
    fn test_simple_codec()
    {
        let mut buf = [0u8; 4096];
        let mut metadata = CodecMetadata::new();
        let count = 100u64;
        let mut measures = Vec::new();
        for i in 0..count {
            measures.push(Measurement{timestamp: 1567029708 + (i * 60) + (i % 3) * 10, count: 1000 + (i % 3), value: 43.568 + (i as f64 * 0.0023456)})
        }

        for i in 0..measures.len() {
            assert_eq!(encode(&mut buf, &mut metadata, measures.get(i).unwrap()).ok(), Some(()));
        }

        println!("Encoded {} measures to {} bytes ({} bytes/measure)", count, metadata.byte_len(), metadata.byte_len()/count as usize);

        let mut metadata2 = CodecMetadata::new();

        for i in 0..measures.len() {
            let result = decode(&buf, &mut metadata2).unwrap();
            assert!(measure_is_close(&result, measures.get(i).unwrap()),
                    "wanted: {:?}, got: {:?}", measures.get(i).unwrap(), result);
        }
    }
}