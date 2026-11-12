use flate2::write::DeflateEncoder;
use flate2::{Compression, Crc};
use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

const DOS_DATE: u16 = (1 << 5) | 1;
const ALIGN: usize = 64;

struct Entry {
    name: String,
    crc: u32,
    packed: u32,
    size: u32,
    offset: u32,
}

pub struct Npz {
    file: BufWriter<File>,
    at: u64,
    entries: Vec<Entry>,
}

struct Counted<'a> {
    file: &'a mut BufWriter<File>,
    at: &'a mut u64,
}

impl Write for Counted<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.file.write(buf)?;
        *self.at += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

pub struct Member<'a> {
    encoder: DeflateEncoder<Counted<'a>>,
    entries: &'a mut Vec<Entry>,
    crc: Crc,
    size: u64,
    name: String,
    offset: u64,
}

fn small(n: u64) -> io::Result<u32> {
    u32::try_from(n).map_err(|_| io::Error::other("the rows file would need zip64"))
}

pub fn npy_header(descr: &str, shape: &[usize]) -> Vec<u8> {
    let dims = match shape {
        [] => "()".to_owned(),
        [n] => format!("({n},)"),
        _ => format!("({})", shape.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")),
    };
    let mut dict = format!("{{'descr': '{descr}', 'fortran_order': False, 'shape': {dims}, }}");
    dict.push_str(&" ".repeat((ALIGN - (10 + dict.len() + 1) % ALIGN) % ALIGN));
    dict.push('\n');
    let mut out = b"\x93NUMPY\x01\x00".to_vec();
    out.extend_from_slice(&(dict.len() as u16).to_le_bytes());
    out.extend_from_slice(dict.as_bytes());
    out
}

pub fn utf32(texts: &[&str]) -> (String, Vec<u8>) {
    let width = texts.iter().map(|t| t.chars().count()).max().unwrap_or(0).max(1);
    let mut bytes = Vec::with_capacity(texts.len() * width * 4);
    for t in texts {
        let start = bytes.len();
        for c in t.chars() {
            bytes.extend_from_slice(&(c as u32).to_le_bytes());
        }
        bytes.resize(start + width * 4, 0);
    }
    (format!("<U{width}"), bytes)
}

impl Npz {
    pub fn create(path: &Path) -> io::Result<Self> {
        Ok(Self { file: BufWriter::with_capacity(1 << 20, File::create(path)?), at: 0, entries: Vec::new() })
    }

    pub fn member(&mut self, name: &str, descr: &str, shape: &[usize]) -> io::Result<Member<'_>> {
        let name = format!("{name}.npy");
        let offset = self.at;
        let mut head = Vec::with_capacity(30 + name.len());
        head.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        head.extend_from_slice(&20u16.to_le_bytes());
        head.extend_from_slice(&0u16.to_le_bytes());
        head.extend_from_slice(&8u16.to_le_bytes());
        head.extend_from_slice(&0u16.to_le_bytes());
        head.extend_from_slice(&DOS_DATE.to_le_bytes());
        head.extend_from_slice(&[0; 12]);
        head.extend_from_slice(&(name.len() as u16).to_le_bytes());
        head.extend_from_slice(&0u16.to_le_bytes());
        head.extend_from_slice(name.as_bytes());
        self.file.write_all(&head)?;
        self.at += head.len() as u64;
        let encoder = DeflateEncoder::new(Counted { file: &mut self.file, at: &mut self.at }, Compression::default());
        let mut member = Member { encoder, entries: &mut self.entries, crc: Crc::new(), size: 0, name, offset };
        member.write(&npy_header(descr, shape))?;
        Ok(member)
    }

    pub fn array(&mut self, name: &str, descr: &str, shape: &[usize], data: &[u8]) -> io::Result<()> {
        let mut member = self.member(name, descr, shape)?;
        member.write(data)?;
        member.end()
    }

    pub fn text(&mut self, name: &str, texts: &[&str], scalar: bool) -> io::Result<()> {
        let (descr, bytes) = utf32(texts);
        let shape: &[usize] = if scalar { &[] } else { &[texts.len()] };
        self.array(name, &descr, shape, &bytes)
    }

    pub fn finish(mut self) -> io::Result<()> {
        let start = self.at;
        let mut dir = Vec::new();
        for e in &self.entries {
            dir.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            dir.extend_from_slice(&20u16.to_le_bytes());
            dir.extend_from_slice(&20u16.to_le_bytes());
            dir.extend_from_slice(&0u16.to_le_bytes());
            dir.extend_from_slice(&8u16.to_le_bytes());
            dir.extend_from_slice(&0u16.to_le_bytes());
            dir.extend_from_slice(&DOS_DATE.to_le_bytes());
            dir.extend_from_slice(&e.crc.to_le_bytes());
            dir.extend_from_slice(&e.packed.to_le_bytes());
            dir.extend_from_slice(&e.size.to_le_bytes());
            dir.extend_from_slice(&(e.name.len() as u16).to_le_bytes());
            dir.extend_from_slice(&[0; 12]);
            dir.extend_from_slice(&e.offset.to_le_bytes());
            dir.extend_from_slice(e.name.as_bytes());
        }
        let size = dir.len() as u64;
        let count = self.entries.len() as u16;
        dir.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        dir.extend_from_slice(&[0; 4]);
        dir.extend_from_slice(&count.to_le_bytes());
        dir.extend_from_slice(&count.to_le_bytes());
        dir.extend_from_slice(&small(size)?.to_le_bytes());
        dir.extend_from_slice(&small(start)?.to_le_bytes());
        dir.extend_from_slice(&0u16.to_le_bytes());
        self.file.write_all(&dir)?;
        self.file.flush()?;
        self.file.get_ref().sync_all()
    }
}

impl Member<'_> {
    pub fn write(&mut self, data: &[u8]) -> io::Result<()> {
        self.crc.update(data);
        self.size += data.len() as u64;
        self.encoder.write_all(data)
    }

    pub fn end(self) -> io::Result<()> {
        let Member { encoder, entries, crc, size, name, offset } = self;
        let counted = encoder.finish()?;
        let end = *counted.at;
        let entry = Entry { crc: crc.sum(), packed: small(end - offset - 30 - name.len() as u64)?, size: small(size)?, offset: small(offset)?, name };
        counted.file.seek(SeekFrom::Start(offset + 14))?;
        counted.file.write_all(&entry.crc.to_le_bytes())?;
        counted.file.write_all(&entry.packed.to_le_bytes())?;
        counted.file.write_all(&entry.size.to_le_bytes())?;
        counted.file.seek(SeekFrom::Start(end))?;
        entries.push(entry);
        Ok(())
    }
}
