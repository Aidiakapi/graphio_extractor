use std::path::{Path, PathBuf};
use std::io::{self, Read, Write};
use std::fs::{self};

#[derive(Debug)]
pub struct FactorioPaths {
    pub executable: PathBuf,
    pub scenarios_directory: PathBuf,
    pub script_output_directory: PathBuf,
}

type Result<T> = std::io::Result<T>;

/// Gets the important paths of the Factorio game.
/// 
/// # Remark
/// Uses `config-path.cfg` to determine in which relative directory to scan for.
/// This isn't a perfect heuristic, as the game itself will create a `config.ini`
/// file on first run. If there's any decent reason that'd warrant complicating
/// this code, a more accuracy solution can be implemented later.
pub fn get_factorio_paths(root_dir: &::std::ffi::OsStr) -> Result<FactorioPaths> {
    let root_dir = canonicalize(root_dir)?;
    let mut executable = root_dir.clone();
    executable.push("bin");
    executable.push("x64");
    executable.push("factorio.exe");

    let mut config_path = root_dir.clone();
    config_path.push("config-path.cfg");

    let mut config_file = fs::File::open(config_path)?;
    let mut config = String::new();
    config_file.read_to_string(&mut config)?;
    let config = config;

    let use_system_data_directory = if config.lines().find(|x| x == &"use-system-read-write-data-directories=true").is_some() {
        true
    } else if config.lines().find(|x| x == &"use-system-read-write-data-directories=false").is_some() {
        false
    }
    else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "cannot get use-system-read-write-data-directories from config-path.cfg"))
    };

    let data_root = if use_system_data_directory {
        canonicalize(get_system_data_directory())?
    }
    else {
        root_dir
    };

    let mut scenarios_directory = data_root.clone();
    scenarios_directory.push("scenarios");
    let mut script_output_directory = data_root;
    script_output_directory.push("script-output");

    Ok(FactorioPaths {
        executable,
        scenarios_directory,
        script_output_directory,
    })
}

pub struct TempDirectory {
    path: PathBuf,
    should_delete: bool,
}

impl TempDirectory {
    pub fn new<P: Into<PathBuf>>(path: P) -> TempDirectory {
        let path = path.into();
        TempDirectory {
            path,
            should_delete: true,
        }
    }

    pub fn ensure<P: Into<PathBuf>>(path: P) -> Result<TempDirectory> {
        let path = path.into();
        let should_delete = ensure_dir(&path)?;
        Ok(TempDirectory {
            path,
            should_delete,
        })
    }

    pub fn path(&self) -> &PathBuf { &self.path }

    pub fn release(&mut self) {
        self.should_delete = false;
    }

    pub fn release_into(mut self) -> PathBuf {
        self.release();
        let mut path = PathBuf::new();
        std::mem::swap(&mut path, &mut self.path);
        path
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        if self.should_delete {
            self.should_delete = false;
            let _ = fs::remove_dir(&self.path);
        }
    }
}

pub struct TempFile {
    path: PathBuf,
    should_delete: bool,
}

impl TempFile {
    pub fn new<P: Into<PathBuf>>(path: P) -> TempFile {
        let path = path.into();
        TempFile {
            path,
            should_delete: true,
        }
    }

    // pub fn path(&self) -> &PathBuf { &self.path }

    // pub fn release(&mut self) {
    //     self.should_delete = false;
    // }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if self.should_delete {
            self.should_delete = false;
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Ensures that a directory exists.
/// 
/// Returns whether the directory had to be created.
pub fn ensure_dir<P: AsRef<Path>>(path: P) -> Result<bool> {
    match fs::create_dir(path) {
        Ok(_) => return Ok(true),
        Err(ref x) if x.kind() == io::ErrorKind::AlreadyExists => return Ok(false),
        Err(x) => return Err(x),
    }
}

pub fn create_dir_safely<P: Into<PathBuf>>(parent: P, directory_name: &str) -> Result<PathBuf> {
    let mut root_path = parent.into();
    let mut dir_name_buf = String::with_capacity(directory_name.len() + 4);
    let mut current_appendix: Option<usize> = None;

    loop {
        dir_name_buf.push_str(directory_name);
        let next_appendix = match current_appendix {
            None => 0,
            Some(x) => {
                dir_name_buf.push('_');
                dir_name_buf.push_str(&x.to_string());
                x + 1
            }
        };
        current_appendix = Some(next_appendix);

        root_path.push(&dir_name_buf);

        if ensure_dir(&root_path)? {
            return Ok(root_path)
        }

        root_path.pop();
        dir_name_buf.clear();
    }
}

/// Writes content to a file, but if that file already exists, it'll
/// not overwrite it, but rather, append a suffix to the file name.
/// 
/// The `extension` parameter should not have a leading `.`
pub fn write_file_safely<P: Into<PathBuf>>(parent: P, file_name: &str, extension: &str, contents: &[u8]) -> Result<PathBuf> {
    let mut root_path = parent.into();
    let mut file_name_buf = String::with_capacity(file_name.len() + extension.len() + 5);
    let mut current_appendix: Option<usize> = None;
    
    loop {
        file_name_buf.push_str(file_name);
        let next_appendix = match current_appendix {
            None => 0,
            Some(x) => {
                file_name_buf.push('_');
                file_name_buf.push_str(&x.to_string());
                x + 1
            },
        };
        current_appendix = Some(next_appendix);
        
        file_name_buf.push('.');
        file_name_buf.push_str(extension);

        root_path.push(&file_name_buf);

        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&root_path) {
            Ok(mut file) => {
                file.write_all(contents)?;
                return Ok(root_path);
            },
            Err(ref e) if e.kind() == io::ErrorKind::AlreadyExists => (),
            Err(err) => return Err(err),
        }

        root_path.pop();
        file_name_buf.clear();
    }
}

fn get_system_data_directory() -> PathBuf {
    // Warning: This code has only been tested on Windows.
    if cfg!(target_os = "windows") {
        let mut buf = dirs::data_dir().unwrap();
        buf.push("Factorio");
        buf
    }
    else if cfg!(target_os = "macos") {
        let mut buf = dirs::data_dir().unwrap();
        buf.push("factorio");
        buf
    }
    else if cfg!(target_os = "linux") {
        let mut buf = dirs::home_dir().unwrap();
        buf.push(".factorio");
        buf
    }
    else {
        // Factorio only runs on Windows, Linux and MacOS
        unreachable!()
    }
}

/// Canonicalizes a path similar to `std::fs::canonicalize`,
/// except that on Windows, it won't convert "regular paths"
/// into UNC-paths.
/// 
/// # Example
/// 
/// ```
/// println!("{:?}", std::fs::canonicalize("C:\Users")); // prints: \\?\C:\Users
/// println!("{:?}", factorio_io::canonicalize("C:\Users")); // prints: C:\Users
/// ```
pub fn canonicalize<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    if cfg!(target_os = "windows") {
        let path = fs::canonicalize(path)?;

        let mut iter = path.into_iter();
        if let Some(entry) = iter.next() {
            let entry = entry.to_string_lossy();
            if entry.starts_with(r"\\?\") && entry.ends_with(':') {
                let mut new_path = PathBuf::new();
                new_path.push(&entry[4..]);
                while let Some(next) = iter.next() {
                    new_path.push(next);
                }
                return Ok(new_path);
            }
        }

        Ok(path)
    }
    else {
        fs::canonicalize(path)
    }
}
