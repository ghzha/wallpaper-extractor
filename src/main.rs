use anyhow::{Ok, Result};
use binary_rw::{BinaryReader, BinaryWriter, Endian, FileStream, SeekStream};
use std::{
    env,
    fs::{create_dir_all, File},
    path::{Path, PathBuf},
};

use wallpaper_extractor::texture::read_texture;

#[derive(Debug, Clone, PartialEq, Eq)]
enum ItemType {
    TexFile,
    OtherFile,
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct PkgItem {
    file_path: PathBuf,
    offset: u32,
    length: u32,
    file_type: ItemType,
}

impl PkgItem {
    fn new(file_path: &str, offset: u32, length: u32) -> Self {
        let path = Path::new(file_path).to_owned();
        let ext = path.extension().unwrap().to_str();
        let file_type = match ext {
            Some("tex") => ItemType::TexFile,
            _ => ItemType::OtherFile,
        };
        Self {
            file_path: path,
            offset: offset,
            length: length,
            file_type: file_type,
        }
    }
}

fn parse_pkg(reader: &mut BinaryReader) -> Result<Vec<PkgItem>> {
    let mut items = Vec::new();
    reader.read_string().expect("Failed to read string");
    let item_num = reader.read_u32()?;
    for _ in 0..item_num {
        let full_path = reader.read_string()?;
        let offset = reader.read_u32()?;
        let length = reader.read_u32()?;
        items.push(PkgItem::new(&full_path, offset, length));
    }
    Ok(items)
}
#[allow(unused)]
fn write_file(
    reader: &mut BinaryReader,
    item: &PkgItem,
    output_dir: &str,
    start: usize,
) -> Result<()> {
    let full_path = Path::new(output_dir).join(&item.file_path);
    let parent = full_path.parent().unwrap();
    if !parent.exists() {
        create_dir_all(parent)?;
    }
    let mut fs = FileStream::new(File::create(full_path)?);
    let mut writer = BinaryWriter::new(&mut fs, Endian::Little);
    reader.seek(item.offset as usize + start)?;
    let bytes = reader.read_bytes(item.length as usize)?;
    writer.write_bytes(bytes)?;
    Ok(())
}

#[allow(unused)]
fn parse(input_file: &str, output_dir: &str) -> Result<()> {
    let mut fs = FileStream::open(input_file).expect("Failed to open file.");
    let mut reader = BinaryReader::new(&mut fs, Endian::Little);
    let items = parse_pkg(&mut reader)?;
    let start = reader.tell()?;
    for item in items.iter() {
        match item.file_type {
            ItemType::OtherFile => {
                // write_item(&mut reader, item, output_dir, start)?;
            }
            ItemType::TexFile => {
                reader.seek(item.offset as usize + start)?;
                let file_name = item
                    .file_path
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();

                let tex = read_texture(&mut reader, file_name.as_str())?;
                match tex {
                    Some(tex) => {
                        tex.save_img("./")?;
                    }
                    None => {}
                }
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let mut args = env::args();
    args.next();

    let input_file = args.next().expect("No file input!");
    if !Path::new(&input_file).is_file() {
        panic!("input is not a file.")
    }
    let output_dir = match args.next() {
        Some(arg) => {
            if Path::new(&arg).is_dir() {
                arg
            } else {
                panic!("output dir is invalid.")
            }
        }
        None => "./".to_string(),
    };
    parse(input_file.as_str(), &output_dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::parse;

    // use super::*;
    #[test]
    fn test_write_item() {
        // let item =
        // let a = [1, 2, 3, 4];
        parse("src/scene.pkg", "./").unwrap();
    }
}
