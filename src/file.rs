use crate::hasher::Hasher;
use crate::metadata::Metadata;
use smallvec::SmallVec;
use std::cell::RefCell;
use std::cmp::max;
use std::cmp::Ordering;
use std::io;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FileSet {
    /// Tracks number of hardlinks from stat to also count unseen links outside scanned dirs
    pub max_hardlinks: u64,
    pub paths: SmallVec<[Box<Path>; 1]>,
}

impl FileSet {
    pub fn new(path: Box<Path>, max_hardlinks: u64) -> Self {
        let mut paths = SmallVec::new();
        paths.push(path);
        FileSet {
            max_hardlinks,
            paths,
        }
    }

    pub fn push(&mut self, path: Box<Path>) {
        self.paths.push(path);
    }

    /// Number of known hardlinks to this file content
    pub fn links(&self) -> u64 {
        max(self.max_hardlinks, self.paths.len() as u64)
    }
}

#[derive(Debug)]
/// File content is efficiently compared using this struct's `PartialOrd` implementation
pub struct FileContent {
    path: Box<Path>,
    metadata: Metadata,
    /// Hashes of content, calculated incrementally
    hashes: RefCell<Hasher>,
}

impl FileContent {
    pub fn from_path(path: Box<Path>) -> Result<Self, io::Error> {
        let m = Metadata::from_path(&path)?;
        Ok(Self::new(path, m))
    }

    pub fn new(path: Box<Path>, metadata: Metadata) -> Self {
        FileContent {
            path,
            metadata,
            hashes: RefCell::new(Hasher::new()),
        }
    }
}

impl Eq for FileContent {}

impl PartialEq for FileContent {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other)
            .map_or(false, |o| o == Ordering::Equal)
    }
}

impl Ord for FileContent {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other).unwrap_or(Ordering::Greater)
    }
}

/// That does the bulk of hasing and comparisons
impl PartialOrd for FileContent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.compare(other).ok()
    }
}

impl FileContent {
    fn compare(&self, other: &Self) -> io::Result<Ordering> {
        // Fast pointer comparison
        if std::ptr::eq(self, other) {
            return Ok(Ordering::Equal);
        }

        // Different file sizes mean they're obviously different.
        // Also different devices mean they're not the same as far as we're concerned
        // (since search is intended for hardlinking and hardlinking only works within the same device).
        let cmp = self.metadata.cmp(&other.metadata);
        if cmp != Ordering::Equal {
            return Ok(cmp);
        }

        let mut hashes1 = self.hashes.borrow_mut();
        let mut hashes2 = other.hashes.borrow_mut();

        hashes1.compare(&mut hashes2, self.metadata.size, &self.path, &other.path)
    }
}
