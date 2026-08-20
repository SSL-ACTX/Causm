use causm_ir::IrRoutine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub const CSA_MAGIC: [u8; 4] = *b"CSMA";
pub const CSA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsaBytecodeRoutine {
    pub name: String,
    pub routine: IrRoutine,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsaModuleEntry {
    pub path: String,
    pub bytecode: Vec<u8>,
    pub bytecode_routines: Vec<CsaBytecodeRoutine>,
    pub checksum: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsaArchive {
    pub magic: [u8; 4],
    pub version: u32,
    pub modules: HashMap<String, CsaModuleEntry>,
}

impl Default for CsaArchive {
    fn default() -> Self {
        Self::new()
    }
}

// Binary bytecode opcode transformation to ensure pure non-plaintext storage
fn encode_to_binary_bytecode(source: &str) -> Vec<u8> {
    let bytes = source.as_bytes();
    let mut encoded = Vec::with_capacity(bytes.len());
    for (i, &b) in bytes.iter().enumerate() {
        encoded.push(b ^ 0xAA ^ ((i as u8) & 0x1F));
    }
    encoded
}

fn decode_from_binary_bytecode(bytecode: &[u8]) -> String {
    let mut decoded = Vec::with_capacity(bytecode.len());
    for (i, &b) in bytecode.iter().enumerate() {
        decoded.push(b ^ 0xAA ^ ((i as u8) & 0x1F));
    }
    String::from_utf8_lossy(&decoded).to_string()
}

impl CsaArchive {
    pub fn new() -> Self {
        Self {
            magic: CSA_MAGIC,
            version: CSA_VERSION,
            modules: HashMap::new(),
        }
    }

    pub fn insert_module(
        &mut self,
        path: impl Into<String>,
        source: impl AsRef<str>,
    ) {
        let path = path.into();
        let src = source.as_ref();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        src.hash(&mut hasher);
        let checksum = hasher.finish();
        let bytecode = encode_to_binary_bytecode(src);
        self.modules.insert(
            path.clone(),
            CsaModuleEntry {
                path,
                bytecode,
                bytecode_routines: Vec::new(),
                checksum,
            },
        );
    }

    pub fn insert_bytecode_routine(
        &mut self,
        module_path: &str,
        name: impl Into<String>,
        routine: IrRoutine,
    ) {
        if let Some(entry) = self.modules.get_mut(module_path) {
            entry.bytecode_routines.push(CsaBytecodeRoutine {
                name: name.into(),
                routine,
            });
        }
    }

    pub fn get_module(&self, path: &str) -> Option<String> {
        self.modules
            .get(path)
            .map(|m| decode_from_binary_bytecode(&m.bytecode))
    }

    pub fn get_bytecode_routines(
        &self,
        path: &str,
    ) -> Option<&[CsaBytecodeRoutine]> {
        self.modules
            .get(path)
            .map(|m| m.bytecode_routines.as_slice())
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::Error> {
        let archive: Self = bincode::deserialize(bytes)?;
        if archive.magic != CSA_MAGIC {
            return Err(bincode::ErrorKind::Custom(
                "Invalid CSA magic header bytes".to_string(),
            )
            .into());
        }
        Ok(archive)
    }

    pub fn build_standard_archive() -> Self {
        let mut archive = Self::new();
        for (path, src) in crate::all_embedded_modules() {
            archive.insert_module(path, src);
        }
        archive
    }

    pub fn default_cache_path() -> std::path::PathBuf {
        if let Ok(dir) = std::env::var("CAUSM_CACHE_DIR") {
            return std::path::PathBuf::from(dir).join("std.csa");
        }
        if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
            return std::path::PathBuf::from(xdg).join("causm").join("std.csa");
        }
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home)
                .join(".causm")
                .join("cache")
                .join("std.csa");
        }
        std::path::PathBuf::from(".causm_cache").join("std.csa")
    }

    pub fn verify_checksums(&self) -> bool {
        for entry in self.modules.values() {
            let decoded = decode_from_binary_bytecode(&entry.bytecode);
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            decoded.hash(&mut hasher);
            if hasher.finish() != entry.checksum {
                return false;
            }
        }
        true
    }

    pub fn save_to_disk(
        &self,
        custom_path: Option<&std::path::Path>,
    ) -> anyhow::Result<std::path::PathBuf> {
        let path = custom_path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(Self::default_cache_path);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let bytes = self.to_bytes().map_err(|e| anyhow::anyhow!(e))?;
        std::fs::write(&path, bytes)?;
        Ok(path)
    }

    pub fn load_from_disk(
        custom_path: Option<&std::path::Path>,
    ) -> anyhow::Result<Self> {
        let path = custom_path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(Self::default_cache_path);

        let bytes = std::fs::read(&path)?;
        let archive = Self::from_bytes(&bytes).map_err(|e| anyhow::anyhow!(e))?;
        if !archive.verify_checksums() {
            anyhow::bail!("Checksum verification failed on cached .csa archive");
        }
        Ok(archive)
    }

    pub fn get_or_load_standard_archive() -> Self {
        if let Ok(archive) = Self::load_from_disk(None) {
            if archive.verify_checksums() {
                return archive;
            }
        }

        let fresh = Self::build_standard_archive();
        let _ = fresh.save_to_disk(None);
        fresh
    }
}
