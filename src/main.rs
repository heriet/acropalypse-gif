use anyhow::Result;
use bitstream_io::{BitRead, BitReader};
use byteorder::ReadBytesExt;
use gif::{Encoder as GifEncoder, Frame, Repeat};
use std::borrow::Cow;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read, SeekFrom};
use weezl::{decode::Decoder as LzwDecoder, BitOrder};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("RestoreError: {0}")]
    RestoreError(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        fail();
    }

    let command = args[1].clone();

    match command.as_str() {
        "detect" => {
            if args.len() < 3 {
                fail();
            }

            let filepath = args[2].clone();
            detect(filepath);
        }
        "restore" => {
            if args.len() < 5 {
                fail();
            }

            let filepath = args[2].clone();
            let width = args[3].parse().unwrap();
            let height = args[4].parse().unwrap();
            restore(filepath, width, height);
        }
        _ => fail(),
    }
}

fn fail() {
    println!("acropalypse-gif detect <filepath>");
    println!("acropalypse-gif restore <filepath> <width> <height>");
    std::process::exit(1);
}

fn detect(filepath: String) {
    let file = File::open(filepath.clone()).unwrap();
    let mut buf = BufReader::new(file);

    // decode gif
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(&mut buf).unwrap();
    while let Some(_frame) = decoder.read_next_frame().unwrap() {}

    let rest = buf.fill_buf().unwrap().len();
    if rest > 0 {
        println!("{}", filepath)
    }
}

fn restore(filepath: String, width: u16, height: u16) {
    let file = File::open(filepath.clone()).unwrap();
    let mut buf = BufReader::new(file);

    // decode gif
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(&mut buf).unwrap();

    let mut frames = vec![];
    while let Some(frame) = decoder.read_next_frame().unwrap() {
        frames.push(frame.clone());
    }

    // pick palette from gif
    let restpre_palette = match &frames[0].palette {
        Some(p) => p.clone(),
        None => decoder.palette().unwrap().to_vec(),
    };

    let mut rest = vec![];
    buf.read_to_end(&mut rest).unwrap();

    let compressed = extract_rest_compressed_data(rest).unwrap();
    let decompressed = decompress_lzw_after_clear_code(compressed).unwrap();

    let pixels = fill_pixels_from_restored(width, height, &decompressed).unwrap();
    let restore_filepath = format!("{}-restored.gif", filepath);
    encode_single_frame_gif(restore_filepath.clone(), width, height, &restpre_palette, &pixels).unwrap();

    println!("{}", restore_filepath);
}

fn extract_rest_compressed_data(rest: Vec<u8>) -> Result<Vec<u8>> {
    let len = rest.len() as u64;
    let mut cur = Cursor::new(rest);

    let sub_image_block_start = search_sub_image_block_start(len, &mut cur)?;

    let mut compressed = vec![];
    compose_sub_image_block(sub_image_block_start, len, &mut cur, &mut compressed);

    Ok(compressed)
}

fn search_sub_image_block_start(len: u64, cur: &mut Cursor<Vec<u8>>) -> Result<u64> {
    // search sub block size = 0xff
    while cur.read_u8()? != 0xff {}
    let mut start = cur.position() - 1;

    while start < len {
        cur.set_position(start);
        if verify_sub_image_block_start(len, cur).is_ok() {
            return Ok(start);
        }
        start += 1;
    }

    Err(Error::RestoreError("Sub Image Block Not Found".to_string()))?
}

fn verify_sub_image_block_start(len: u64, cur: &mut Cursor<Vec<u8>>) -> Result<()> {
    // search sub block size = 0xff
    while cur.read_u8()? != 0xff {}

    cur.set_position(cur.position() + 0xff);

    while cur.position() < len {
        let block_size = cur.read_u8()?;
        if block_size == 0 {
            break;
        }
        cur.set_position(cur.position() + block_size as u64);
    }

    // Currently, assumed 1 frame and no extention gif image
    // so, next byte is trailer = 0x3b
    let trailer = cur.read_u8()?;
    if trailer == 0x3b {
        Ok(())
    } else {
        Err(Error::RestoreError("Trailer Not Found".to_string()))?
    }
}

fn compose_sub_image_block(start: u64, len: u64, cur: &mut Cursor<Vec<u8>>, dest: &mut Vec<u8>) {
    cur.set_position(start);

    while cur.position() < len {
        let block_size = cur.read_u8().unwrap();
        if block_size == 0 {
            break;
        }
        let mut buf = vec![0u8; block_size as usize];
        cur.read_exact(&mut buf).unwrap();

        dest.append(&mut buf);
    }
}

fn decompress_lzw_after_clear_code(compressed: Vec<u8>) -> Result<Vec<u8>> {
    let compressed_len = compressed.len();
    let compressed_bit_len = compressed_len * 8;
    let mut compressed_cur = Cursor::new(compressed);

    let lzw_minimum_code_size: u8 = 0x08; // Windows Snipping tool and any software may save 0x08
    let max_code_length: usize = 12; // Gif max code length = 12 bit
    let mut offset_bit = 0;

    let mut bit_reader = BitReader::endian(compressed_cur.clone(), bitstream_io::LittleEndian);

    while offset_bit + max_code_length < compressed_bit_len {
        let start_bit = search_start_bit_by_clear_code(
            lzw_minimum_code_size,
            offset_bit,
            compressed_len,
            &mut compressed_cur,
        )?;
        bit_reader.seek_bits(SeekFrom::Start(start_bit as u64))?;
        bit_reader.skip(max_code_length as u32)?; // skip searched clear code bits

        // bit_reader to vec
        let bit_pos = start_bit + max_code_length;
        let bit_buf_size = if bit_pos % 8 == 0 {
            compressed_len - bit_pos / 8
        } else {
            compressed_len - bit_pos / 8 - 1
        };
        let bit_buf = bit_reader.read_to_vec(bit_buf_size)?;

        // decode lzw
        let lzw_result = LzwDecoder::new(BitOrder::Lsb, lzw_minimum_code_size).decode(&bit_buf);
        match lzw_result {
            Ok(decompressed) => {
                if compressed_len < decompressed.len() {
                    // decompressed lzw will larger than compressed_len
                    return Ok(decompressed);
                } else {
                    offset_bit = start_bit + 1;
                }
            }
            Err(_) => {
                offset_bit = start_bit + 1;
            }
        }
    }

    Err(Error::RestoreError("decompress lzw failed".to_string()))?
}

fn search_start_bit_by_clear_code(
    code_size: u8,
    offset_bit: usize,
    len: usize,
    cur: &mut Cursor<Vec<u8>>,
) -> Result<usize> {
    let clear_code: u16 = 1 << code_size;
    let max_code_length: usize = 12; // Gif max code length = 12 bit

    let len_bit = (len * 8) as u64;
    let mut start = offset_bit;
    let mut reader = BitReader::endian(cur, bitstream_io::LittleEndian);

    while start + max_code_length < len_bit as usize {
        reader.seek_bits(SeekFrom::Start(start as u64)).unwrap();
        let code = reader.read::<u16>(max_code_length as u32).unwrap();

        if code == clear_code {
            return Ok(start);
        }

        start += 1;
    }

    Err(Error::RestoreError("Clear Code Not Found".to_string()))?
}

fn fill_pixels_from_restored(width: u16, height: u16, restored: &[u8]) -> Result<Vec<u8>> {
    let mut pixels = vec![];

    let image_length = width as usize * height as usize;
    let empty_length = image_length - restored.len();

    for i in 0..image_length {
        if i < empty_length {
            pixels.push(0x00);
        } else {
            pixels.push(restored[i - empty_length]);
        }
    }

    Ok(pixels)
}

fn encode_single_frame_gif(
    filepath: String,
    width: u16,
    height: u16,
    global_palette: &[u8],
    pixels: &[u8],
) -> Result<()> {
    let mut file = File::create(filepath)?;

    let mut encoder = GifEncoder::new(&mut file, width, height, global_palette)?;
    encoder.set_repeat(Repeat::Infinite).unwrap();

    let mut frame = Frame::<'_> {
        width,
        height,
        ..Default::default()
    };
    frame.buffer = Cow::Borrowed(pixels);
    encoder.write_frame(&frame)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case01_compose_sub_image_block() {
        let mut file = File::open("asset/case01.gif").unwrap();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();

        let len = buf.len();
        let mut cur = Cursor::new(buf);
        let mut compressed = vec![];
        compose_sub_image_block(800, len as u64, &mut cur, &mut compressed);

        let decompressed = LzwDecoder::new(BitOrder::Lsb, 0x08).decode(&compressed);
        assert!(decompressed.is_ok());
    }

    #[test]
    fn case01_decompress_from_fix_pos() {
        let mut file = File::open("asset/case01.gif").unwrap();
        let mut buf = vec![];
        file.read_to_end(&mut buf).unwrap();

        let len = buf.len();
        let mut cur = Cursor::new(buf);
        let mut compressed = vec![];
        compose_sub_image_block(800, len as u64, &mut cur, &mut compressed);

        let compressed_len = compressed.len();
        let compressed_cur = Cursor::new(compressed);
        let mut bit_reader = BitReader::endian(compressed_cur, bitstream_io::LittleEndian);

        let bit_pos_start_clear_code = 216224;
        bit_reader
            .seek_bits(SeekFrom::Start(bit_pos_start_clear_code as u64 - 12))
            .unwrap();

        let clear_code = bit_reader.read::<u16>(12).unwrap();
        assert_eq!(clear_code, 256);

        let lacked = bit_reader
            .read_to_vec(compressed_len - bit_pos_start_clear_code / 8)
            .unwrap();

        let decompressed = LzwDecoder::new(BitOrder::Lsb, 0x08).decode(&lacked);
        assert!(decompressed.is_ok());
    }
}
