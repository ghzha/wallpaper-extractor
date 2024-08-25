use crate::enums::{image2mipmap, FreeImageFormat, MipmapFormat, TexFlags, TexFormat};
use anyhow::Result;
use binary_rw::BinaryReader;
use image::ImageReader;
use lz4_flex::block::decompress;
use std::{io::Cursor, path::Path};

trait StringReaderExt {
    fn read_n_string(&mut self, max_length: usize) -> Result<String>;
}
impl<'a> StringReaderExt for BinaryReader<'a> {
    fn read_n_string(&mut self, n: usize) -> Result<String> {
        let bytes = self.read_bytes(n)?;
        Ok(String::from_utf8(bytes)?)
    }
}

#[allow(unused)]
#[derive(Debug)]
pub struct TexHeader {
    format: TexFormat,
    flags: TexFlags,
    tex_width: u32,
    tex_height: u32,
    img_width: u32,
    img_height: u32,
    unk_int0: i32,
}
#[allow(unused)]
#[derive(Debug)]
pub struct TexMipmap {
    width: u32,
    height: u32,
    lz4_compressed: bool,
    decompressed_bytes_count: usize,
    bytes: Vec<u8>,
    version: u8,
    format: MipmapFormat,
}
#[allow(unused)]
#[derive(Debug)]
struct TexImage {
    mipmaps: Vec<TexMipmap>,
}
#[allow(unused)]
#[derive(Debug)]
struct TexImageContainer {
    magic: String,
    image_format: FreeImageFormat,
    images: Vec<TexImage>,
    version: u8,
}

#[allow(unused)]
#[derive(Debug)]
pub struct Tex {
    name: String,
    magic1: String,
    magic2: String,
    header: TexHeader,
    image_container: TexImageContainer,
    // frame_info_container: i8,
    is_gif: bool, // (header.Flags & flag) == flag;
}

impl Tex {
    pub fn save_img(self: &Self, path: &str) -> Result<()> {
        match self.image_container.image_format {
            FreeImageFormat::FIF_UNKNOWN => {}
            _ => {
                let mipmap = self
                    .image_container
                    .images
                    .get(0)
                    .unwrap()
                    .mipmaps
                    .get(0)
                    .unwrap();
                let img_reader = ImageReader::new(Cursor::new(&mipmap.bytes));
                match img_reader.with_guessed_format() {
                    Result::Ok(img) => {
                        let format = img.format().unwrap().extensions_str()[0];
                        let save_path = Path::new(path).join(format!("{}.{}", &self.name, format));
                        let img = img.decode()?;
                        img.save(&save_path)?;
                        println!("Save to {:?}", save_path);
                    }
                    Err(e) => {
                        eprintln!("{:?}, failed to parse {:?}", e, self.name);
                    }
                }
            }
        };
        Ok(())
    }
}

impl TexHeader {
    fn new(reader: &mut BinaryReader) -> Result<Self> {
        let format = reader.read_u32()?;
        let tex_format = TexFormat::try_from(format).expect("Invalid Texture format.");
        let flag = reader.read_i32()?;
        let tex_flags = TexFlags::try_from(flag).expect("Invalid texture header flag.");
        let tex_width = reader.read_u32()?;
        let tex_height = reader.read_u32()?;
        let img_width = reader.read_u32()?;
        let img_height = reader.read_u32()?;
        let unk_int0 = reader.read_i32()?;
        Ok(TexHeader {
            format: tex_format,
            flags: tex_flags,
            tex_width: tex_width,
            tex_height: tex_height,
            img_width: img_width,
            img_height: img_height,
            unk_int0: unk_int0,
        })
    }
}

fn read_mipmap_v1(reader: &mut BinaryReader, format: MipmapFormat) -> Result<TexMipmap> {
    let width = reader.read_u32()?;
    let height = reader.read_u32()?;
    let bytes = read_bytes(reader)?;
    Ok(TexMipmap {
        width: width,
        height: height,
        lz4_compressed: false,
        decompressed_bytes_count: 0,
        bytes: bytes,
        version: 1,
        format: format,
    })
}
fn read_mipmap_v2v3(
    reader: &mut BinaryReader,
    version: u8,
    format: MipmapFormat,
) -> Result<TexMipmap> {
    let width = reader.read_u32()?;
    let height = reader.read_u32()?;
    let lz4_compressed = reader.read_u32()?;
    let lz4_compressed = lz4_compressed == 1;
    let decompressed_bytes_count = reader.read_u32()?;
    let bytes = read_bytes(reader)?;
    Ok(TexMipmap {
        width: width,
        height: height,
        lz4_compressed: lz4_compressed,
        decompressed_bytes_count: decompressed_bytes_count as usize,
        bytes: bytes,
        version: version,
        format: format,
    })
}
// fn lz4_decompress(bytes: Vec<u8>, length: usize) -> Vec<u8> {
//     // let buffer = [0; length];
//     let result = decompress(&bytes, length)?;
// }

fn decompress_mipmap(mipmap: &mut TexMipmap) {
    if mipmap.lz4_compressed {
        mipmap.bytes = decompress(mipmap.bytes.as_slice(), mipmap.decompressed_bytes_count)
            .expect("failed to decompress mipmap.");
        mipmap.lz4_compressed = false;
    }
    let format_val: u32 = mipmap.format.clone().into();
    if format_val >= 1000 {
        return;
    }
    match mipmap.format {
        MipmapFormat::CompressedDXT5 => {
            // let output =
            // mipmap.bytes = Format::Bc3.decompress(mipmap.bytes, mipmap.width, mipmap.height, output);
            mipmap.format = MipmapFormat::RGBA8888;
        }
        MipmapFormat::CompressedDXT3 => {
            mipmap.format = MipmapFormat::RGBA8888;
        }
        MipmapFormat::CompressedDXT1 => {
            mipmap.format = MipmapFormat::RGBA8888;
        }
        _ => {}
    }
}
fn read_bytes(reader: &mut BinaryReader) -> Result<Vec<u8>> {
    let byte_count = reader.read_u32()?;
    let bytes = reader.read_bytes(byte_count as usize)?;
    Ok(bytes)
}

fn read_image(reader: &mut BinaryReader, version: u8, format: MipmapFormat) -> Result<TexImage> {
    // read img
    let mut mipmaps: Vec<TexMipmap> = Vec::new();
    let mipmap_count = reader.read_u32()?;
    // println!("mipmap count: {}", mipmap_count);
    for _ in 0..mipmap_count {
        let mipmap = match version {
            1 => read_mipmap_v1(reader, format.clone()),
            2 | 3 => read_mipmap_v2v3(reader, version, format.clone()),
            _ => panic!("Tex image container version: {} is not supported!", version),
        };
        let mut mipmap = mipmap?;
        decompress_mipmap(&mut mipmap);
        mipmaps.push(mipmap);
        // decompress_mipmap(&mipmap.unwrap());
    }
    Ok(TexImage { mipmaps: mipmaps })
}

fn get_mipmap_format(image_format: &FreeImageFormat, tex_format: &TexFormat) -> MipmapFormat {
    if *image_format != FreeImageFormat::FIF_UNKNOWN {
        return image2mipmap(image_format);
    }
    match *tex_format {
        TexFormat::RGBA8888 => MipmapFormat::RGBA8888,
        TexFormat::DXT5 => MipmapFormat::CompressedDXT5,
        TexFormat::DXT3 => MipmapFormat::CompressedDXT3,
        TexFormat::DXT1 => MipmapFormat::CompressedDXT1,
        TexFormat::R8 => MipmapFormat::R8,
        TexFormat::RG88 => MipmapFormat::RG88,
    }
}

fn read_image_container(
    reader: &mut BinaryReader,
    tex_format: TexFormat,
) -> Result<TexImageContainer> {
    // handle container
    let mut magic = reader.read_n_string(8)?;
    reader.read_u8()?;

    let img_count = reader.read_u32()?;
    let format = match magic.as_str() {
        "TEXB0003" => reader.read_i32()?,
        "TEXB0001" | "TEXB0002" => -1,
        _ => panic!("Unknown Iamge Magic"),
    };
    let conatiner_format = FreeImageFormat::try_from(format)?;
    let container_version = magic.pop().unwrap().to_string().parse::<u8>()?;

    let mipmap_format = get_mipmap_format(&conatiner_format, &tex_format);

    let mut images: Vec<TexImage> = Vec::new();
    for _ in 0..img_count {
        let image = read_image(reader, container_version, mipmap_format.clone())?;
        images.push(image);
    }
    Ok(TexImageContainer {
        magic: magic,
        image_format: conatiner_format,
        images: images,
        version: container_version,
    })
}

pub fn read_texture(reader: &mut BinaryReader, file_name: &str) -> Result<Option<Tex>> {
    let magic1 = reader.read_n_string(8)?;
    if magic1 != "TEXV0005" {
        return Ok(Option::None);
    }
    reader.read_u8()?;

    // test magic2
    let magic2 = reader.read_n_string(8)?;
    if magic2 != "TEXI0001" {
        return Ok(Option::None);
    }
    reader.read_u8()?;

    let header = TexHeader::new(reader)?;
    // println!("Header: {:?}", header);

    if header.flags == TexFlags::IsGif {
        return Ok(Option::None);
    }

    let container = read_image_container(reader, header.format.clone())?;
    // let tex_name = file_name.;
    Ok(Option::Some(Tex {
        name: file_name.to_string(),
        magic1: magic1,
        magic2: magic2,
        header: header,
        image_container: container,
        is_gif: false,
    }))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use binary_rw::{Endian, FileStream};
    use image::ImageReader;

    use super::*;
    #[test]
    fn test_parse_tex() {
        let mut fs = FileStream::open("output/materials/玉子.tex").expect("Failed to open file.");
        let mut reader = BinaryReader::new(&mut fs, Endian::Little);
        let tex = read_texture(&mut reader, "玉子").expect("failed to handle tex file");
        match tex {
            Some(tex) => {
                let mipmap = tex
                    .image_container
                    .images
                    .get(0)
                    .unwrap()
                    .mipmaps
                    .get(0)
                    .unwrap();
                let img = ImageReader::new(Cursor::new(&mipmap.bytes))
                    .with_guessed_format()
                    .unwrap()
                    .decode()
                    .unwrap();
                img.save("./").expect("failed to save image.");
            }
            None => return,
        };
    }
    #[test]
    fn test_enum() {
        let invalid = FreeImageFormat::try_from(25);
        println!("{:?}", invalid.unwrap())
    }
}
