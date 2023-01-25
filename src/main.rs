use anyhow::{Result, Ok};
use clap::Parser;
use rayon::prelude::*;
use std::{fs::{File, create_dir_all}, path::PathBuf, sync::{Arc, Mutex}, io::{Read, Seek, SeekFrom}};

// Unzip all files within a zip file as quickly as possible



// File cannot be cloned so we should use Arc and Mutex to safely clone the File
#[derive(Clone)]
struct CloneableFile{
    file:Arc<Mutex<File>>,
    pos:u64,
    // TODO determine and store this once instead of per cloneable file
    file_length:Option<u64>
}

impl CloneableFile {
    fn new(file:File) -> Self{
        Self { file: Arc::new(Mutex::new(file)), pos: 0u64, file_length: None }
    }    
}

impl CloneableFile{
    fn ascertain_file_length(&mut self)->u64{
        match self.file_length {
            Some(file_length) => file_length,
            None => {
                let len = self.file.lock().unwrap().metadata().unwrap().len();
                self.file_length = Some(len);
                len
            }
        }
    }
}

impl Read for CloneableFile{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut underlying_file = self.file.lock().expect("Unable to get the underlying file");
        underlying_file.seek(SeekFrom::Start(self.pos))?;
        let read_result = underlying_file.read(buf);
        if let std::io::Result::Ok(bytes_read) = read_result {
            // TODO, once stabilised, use checked_add_signed
            self.pos += bytes_read as u64;
        }
        read_result
    }
}

impl Seek for CloneableFile{
    fn seek(&mut self, pos:SeekFrom)-> std::io::Result<u64>{
        let new_pos = match pos {
            SeekFrom::Start(pos) => pos,
            SeekFrom::End(offset_from_end)=>{
                let file_len = self.ascertain_file_length();
                // TODO, once stabilised, use checked_add_signed
                file_len - (-offset_from_end as u64)
            }
            // TODO, once stabilised, use checked_add_signed
            SeekFrom::Current(offset_from_pos) => {
                if offset_from_pos == 0 {
                    self.pos
                } else if offset_from_pos > 0 {
                    self.pos + (offset_from_pos as u64)
                } else {
                    self.pos - ((-offset_from_pos) as u64)
                }
            }
        };
        self.pos = new_pos;
        std::io::Result::Ok(new_pos)
    }
}

#[derive(Parser,Debug)]
#[command(author,version,about,long_about = None)]
struct Args{
    // Zip file to unzip
    #[arg(value_name="FILE")]
    zipfile:PathBuf
}


fn main()->Result<()> {
   let args = Args::parse();
   let zipfile = File::open(args.zipfile)?;
   let zipfile = CloneableFile::new(zipfile);
   let zip = zip::ZipArchive::new(zipfile)?;
   let file_count = zip.len();
   println!("Zip has {} files",file_count);
   (0..file_count).into_par_iter().for_each(|i|{
    let mut myzip = zip.clone();
    let mut file = myzip.by_index(i).expect("Unable to get file from zip");
    if file.is_dir(){
        return;
    }
    let out_file = file.enclosed_name().unwrap();
    println!("Filename: {}",out_file.display());
    if let Some(parent) = out_file.parent(){
        create_dir_all(parent).unwrap_or_else(|err|{
            panic!(
                "Unable to create parent directories for {}: {}",
                out_file.display(),
                err
            )
        });
    }
    let mut out_file = File::create(out_file).unwrap();
    std::io::copy(&mut file, &mut out_file).unwrap();
   });
   Ok(())
}
