use crate::log;
use anyhow::{anyhow, Context};
use flate2::bufread::ZlibDecoder;
use std::{
    fmt::Display,
    fs::File,
    io::{self, BufRead, BufReader, Read},
    str,
    str::FromStr,
};

#[derive(Debug, PartialEq)]
pub enum ObjKind {
    Blob,
    Tree,
    Commit,
}

impl Display for ObjKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjKind::Blob => write!(f, "blob"),
            ObjKind::Tree => write!(f, "tree"),
            ObjKind::Commit => write!(f, "commit"),
        }
    }
}

impl FromStr for ObjKind {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "blob" => Ok(ObjKind::Blob),
            "tree" => Ok(ObjKind::Tree),
            "commit" => Ok(ObjKind::Tree),
            _ => Err(anyhow!(log!("Unrecognized filetype: {s}"))),
        }
    }
}

pub struct Object<R: BufRead> {
    pub kind: ObjKind,
    pub size: u64,
    pub content: R,
}

pub fn read_obj_file(hash: &String) -> anyhow::Result<Object<impl BufRead>> {
    let folder = &hash[..2];
    let file_name = &hash[2..];
    let file = File::open(format!(".git/objects/{}/{}", folder, file_name))
        .with_context(|| log!("unable to open file - {:?}", hash))?;

    let reader = BufReader::new(file);
    let decoder = ZlibDecoder::new(reader);
    let mut reader = BufReader::new(decoder);

    let mut buf = Vec::new();
    reader
        .read_until(0, &mut buf)
        .expect(log!("unable to read header part of object file"));

    let (kind, size) = str::from_utf8(&buf[..buf.len() - 1])
        .with_context(|| log!("Unable to read file type and size: {:?}", buf))?
        .split_once(' ')
        .ok_or_else(|| anyhow!(log!("Header should be two parts - {:?}", buf)))
        .and_then(|(kind, size)| {
            let kind =
                ObjKind::from_str(kind).with_context(|| log!("Invalid file type: {}", kind))?;
            let size = size
                .parse::<u64>()
                .with_context(|| log!("Invalid size: {}", size))?;
            Ok((kind, size))
        })?;
    let content = reader.take(size);
    Ok(Object {
        kind,
        size,
        content,
    })
}

pub fn print_blob_obj(mut object: Object<impl BufRead>) -> anyhow::Result<()> {
    anyhow::ensure!(
        object.kind == ObjKind::Blob,
        log!("Duh!! you need a blob object type here")
    );
    let mut stdout = io::stdout();
    let _ = stdout.lock();
    let n = io::copy(&mut object.content, &mut stdout)
        .context(log!("unable to copy content to stdin"))?;
    anyhow::ensure!(
        n == object.size,
        log!(
            "size written({}) doesn't match size defined({})",
            object.size,
            n
        )
    );
    Ok(())
}

pub fn print_tree_obj(mut object: Object<impl BufRead>, name_only: bool) -> anyhow::Result<()> {
    anyhow::ensure!(object.kind == ObjKind::Tree, log!("Not a tree kind"));
    let stdout = io::stdout();
    let _ = stdout.lock();
    loop {
        let mut mode_and_name = Vec::new();
        let read_count = object.content.read_until(0, &mut mode_and_name)?;
        if read_count == 0 {
            return Ok(());
        }
        let (mode, name) = str::from_utf8(&mode_and_name[..read_count - 1])?
            .split_once(" ")
            .ok_or_else(|| anyhow!(log!("tree header should have mode and filename")))
            .and_then(|(mode, name)| {
                let mode = mode.parse::<usize>()?;
                Ok((mode, name))
            })?;

        let mut hash_buf = [0; 20];
        object.content.read_exact(&mut hash_buf)?;

        // Printing this after reading hash, as we still need to consume those
        if name_only {
            println!("{}", name);
            continue;
        }
        let hash_buf = &hash_buf[..];
        let hash = hash_buf.to_hex_string();

        let object =
            read_obj_file(&hash).with_context(|| log!("Unable to hash found on tree: {}", hash))?;
        println!("{:0>6} {} {}\t{}", mode, object.kind, hash, name);
    }
}

pub trait ToHex {
    fn to_hex_string(&self) -> String;
}

impl ToHex for &[u8] {
    fn to_hex_string(&self) -> String {
        self.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

mod macros {
    #[macro_export]
    macro_rules! log {
        ($msg:literal) => {
            concat!("[", file!(), ":", line!(), "] ", $msg)
        };
        ($fmt:expr, $($arg:tt)*) => {
            format!("{} {}", concat!("[", file!(), ":", line!(), "] "), format!($fmt, $($arg)*))
        }
    }
}
