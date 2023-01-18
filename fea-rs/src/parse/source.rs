//! source files

use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fmt::Debug,
    num::NonZeroU32,
    ops::Range,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use crate::util;

/// Uniquely identifies a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct FileId(NonZeroU32);

/// A single source file, corresponding to a file on disk.
///
/// We keep hold of all sources used in a given compilation so that we can
/// do error reporting.
#[derive(Clone, Debug)]
pub struct Source {
    id: FileId,
    /// The non-canonicalized path to this source, suitable for printing.
    path: OsString,
    contents: Arc<str>,
    /// The index of each newline character, for efficiently fetching lines
    /// (for error reporting, e.g.)
    line_offsets: Arc<[usize]>,
}

/// A list of sources in a project.
#[derive(Clone, Debug)]
pub struct SourceList {
    resolver: Rc<dyn SourceResolver>,
    ids: HashMap<OsString, FileId>,
    sources: HashMap<FileId, Source>,
}

/// A map from positions in a resolved token tree (which may contain the
/// contents of multiple files) to locations in specific files.
#[derive(Clone, Debug, Default)]
pub struct SourceMap {
    /// sorted vec of (offset_in_combined_tree, (file_id, offest_in_source_file));
    offsets: Vec<(Range<usize>, (FileId, usize))>,
}

/// An error that occurs when trying to read a file from disk.
#[derive(Clone, Debug, thiserror::Error)]
#[error("Failed to load source at '{}': '{cause}'", Path::new(.path.as_os_str()).display())]
pub struct SourceLoadError {
    #[source]
    cause: Rc<dyn std::error::Error>,
    path: OsString,
}

/// A trait that abstracts resolving a path.
///
/// In general, paths are resolved through the filesystem; however if you are
/// doing something fancy (such as keeping your source files in memory) you
/// can pass a closure or another custom implementation of this trait into the
/// appropriate parse functions.
///
/// If you need a custom resolver, you can either implement this trait for some
/// custom type, or you can use a closure with the signature,
/// `|&OsStr| -> Result<String, SourceLoadError>`.
pub trait SourceResolver {
    /// Return the contents of the utf-8 encoded file at the provided path.
    fn get_contents(&self, path: &OsStr) -> Result<String, SourceLoadError>;

    /// Given a raw path (the `$path` in `include($path)`), return the path to load.
    ///
    /// See [fncluding files][] for more information. This method is only
    /// relevant when working with the file system.
    ///
    /// The default implementation returns the `path` argument, unchanged.
    ///
    /// [including files]: http://adobe-type-tools.github.io/afdko/OpenTypeFeatureFileSpecification.html#3-including-files
    fn resolve_raw_path(&self, path: &OsStr, _included_from: Option<&OsStr>) -> OsString {
        path.to_owned()
    }

    /// If necessary, canonicalize this path.
    ///
    /// There are an unbounded number of ways to represent a given path;
    /// fot instance, the path `./features.fea` may be equivalent to the path
    /// `./some_folder/../features.fea` or to `../../my/font/features.fea`.
    /// This method is an opportunity to specify the canonical representaiton
    /// of a path.
    fn canonicalize(&self, path: &OsStr) -> Result<OsString, SourceLoadError> {
        Ok(path.to_owned())
    }

    /// A convenience method for creating a `Source` after loading a path.
    fn resolve(&self, path: &OsStr) -> Result<Source, SourceLoadError> {
        let contents = self.get_contents(path)?;
        Ok(Source::new(path.to_owned(), contents.into()))
    }

    // a little helper used in our debug impl
    #[doc(hidden)]
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

impl std::fmt::Debug for dyn SourceResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.type_name().fmt(f)
    }
}

impl<F> SourceResolver for F
where
    F: Fn(&OsStr) -> Result<String, SourceLoadError>,
{
    fn get_contents(&self, path: &OsStr) -> Result<String, SourceLoadError> {
        (self)(path)
    }
}

/// An implementation of [`SourceResolver`] for the local file system.
///
/// This is the common case.
pub(crate) struct FileSystemResolver {
    project_root: PathBuf,
}

impl FileSystemResolver {
    pub(crate) fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }
}

impl SourceResolver for FileSystemResolver {
    fn get_contents(&self, path: &OsStr) -> Result<String, SourceLoadError> {
        std::fs::read_to_string(path).map_err(|cause| SourceLoadError::new(path.into(), cause))
    }

    fn resolve_raw_path(&self, path: &OsStr, included_from: Option<&OsStr>) -> OsString {
        let path = Path::new(path);
        let included_from = included_from.map(Path::new).and_then(Path::parent);
        util::paths::resolve_path(path, &self.project_root, included_from).into_os_string()
    }

    fn canonicalize(&self, path: &OsStr) -> Result<OsString, SourceLoadError> {
        std::fs::canonicalize(path)
            .map_err(|io_err| SourceLoadError::new(path.into(), io_err))
            .map(PathBuf::into_os_string)
    }
}

impl FileId {
    /// A reserved FileId used during parsing.
    pub(crate) const CURRENT_FILE: FileId = FileId(unsafe { NonZeroU32::new_unchecked(1) });

    pub(crate) fn next() -> FileId {
        use std::sync::atomic;
        static COUNTER: atomic::AtomicU32 = atomic::AtomicU32::new(2);
        FileId(NonZeroU32::new(COUNTER.fetch_add(1, atomic::Ordering::Relaxed)).unwrap())
    }
}

impl Source {
    pub(crate) fn new(path: impl Into<OsString>, contents: Arc<str>) -> Self {
        let line_offsets = line_offsets(&contents);
        Source {
            path: path.into(),
            id: FileId::next(),
            contents,
            line_offsets,
        }
    }

    /// The raw text for this source
    pub fn text(&self) -> &str {
        &self.contents
    }

    /// The source's path.
    ///
    /// If the source is a file, this will be the *resolved* file path. In other
    /// cases the exact behaviour depends on the implementation of the current
    /// [`SourceResolver`].
    pub fn path(&self) -> &OsStr {
        &self.path
    }

    /// The [`FileId`] for this source.
    pub fn id(&self) -> FileId {
        self.id
    }

    /// Compute the line and column for a given utf-8 offset.
    pub fn line_col_for_offset(&self, offset: usize) -> (usize, usize) {
        let offset_idx = match self.line_offsets.binary_search(&offset) {
            Ok(x) => x,
            Err(x) => x - 1, // cannot underflow as 0 is always in list
        };
        let offset_of_line = self.line_offsets[offset_idx];
        let offset_in_line = offset - offset_of_line;
        (offset_idx + 1, offset_in_line)
    }

    /// returns the (1-indexed) number and text.
    pub fn line_containing_offset(&self, offset: usize) -> (usize, &str) {
        let offset_idx = match self.line_offsets.binary_search(&offset) {
            Ok(x) => x,
            Err(x) => x - 1, // cannot underflow as 0 is always in list
        };
        let start_offset = self.line_offsets[offset_idx];
        let end_offset = self
            .line_offsets
            .get(offset_idx + 1)
            .copied()
            .unwrap_or(self.contents.len());

        (
            offset_idx + 1,
            self.contents[start_offset..end_offset].trim_end_matches('\n'),
        )
    }

    /// Return the offset of the start of the (1-indexed) line.
    ///
    /// Panics if the line number exceeds the total number of lines in the file.
    pub fn offset_for_line_number(&self, line_number: usize) -> usize {
        self.line_offsets[line_number - 1]
    }
}

fn line_offsets(text: &str) -> Arc<[usize]> {
    // we could use memchar for this; benefits would require benchmarking
    let mut result = vec![0];
    result.extend(
        text.bytes()
            .enumerate()
            .filter_map(|(i, b)| if b == b'\n' { Some(i + 1) } else { None }),
    );
    result.into()
}

impl SourceMap {
    pub(crate) fn add_entry(&mut self, src: Range<usize>, dest: (FileId, usize)) {
        if !src.is_empty() {
            self.offsets.push((src, dest));
        }
    }

    /// panics if `global_range` crosses a file barrier?
    pub(crate) fn resolve_range(&self, global_range: Range<usize>) -> (FileId, Range<usize>) {
        // it is hard to imagine more than a couple hundred include statements,
        // and even that would be extremely rare, so I don't think it's really
        // worth doing a binary search here?
        let (chunk, (file, local_offset)) = self
            .offsets
            .iter()
            .find(|item| item.0.contains(&global_range.start))
            .unwrap();
        let chunk_offset = global_range.start - chunk.start;
        let range_start = *local_offset + chunk_offset;
        let len = global_range.end - global_range.start;
        (*file, range_start..range_start + len)
    }
}

impl SourceList {
    pub(crate) fn new(resolver: impl SourceResolver + 'static) -> Self {
        SourceList {
            ids: Default::default(),
            sources: Default::default(),
            resolver: Rc::new(resolver),
        }
    }

    pub(crate) fn get(&self, id: &FileId) -> Option<&Source> {
        self.sources.get(id)
    }

    /// Attempt to load the source at the provided path.
    ///
    /// This uses the [`SourceResolver`] that was passed in at construction time,
    /// and is used to load both the root source as well as any sources that are
    /// referenced by `include($path)` statements. In this case, the `path` argument
    /// is the literal (e.g. unresolved and uncanonicalized) `$path` in the
    /// include.
    ///
    /// If the source cannot be resolved, returns an error.
    pub(crate) fn source_for_path(
        &mut self,
        path: &dyn AsRef<OsStr>,
        included_by: Option<FileId>,
    ) -> Result<FileId, SourceLoadError> {
        let included_by = included_by.map(|id| self.sources.get(&id).unwrap().path.as_os_str());
        let path = self.resolver.resolve_raw_path(path.as_ref(), included_by);
        let canonical = self.resolver.canonicalize(&path)?;

        if let Some(src) = self.ids.get(&canonical) {
            return Ok(*src);
        }

        let source = self.resolver.resolve(&path)?;
        let id = source.id;
        self.ids.insert(canonical, id);
        self.sources.insert(id, source);
        Ok(id)
    }
}

impl SourceLoadError {
    pub(crate) fn new(path: OsString, cause: impl std::error::Error + 'static) -> Self {
        Self {
            cause: Rc::new(cause),
            path,
        }
    }
}
