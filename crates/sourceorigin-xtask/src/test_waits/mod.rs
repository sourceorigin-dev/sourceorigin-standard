//! `test-waits`: tests wait for events, not time. Flags each call in test
//! code to `std::thread::sleep`, `tokio::time::sleep` or
//! `tokio::time::sleep_until`.
//!
//! Test code is a file named `tests.rs` or under a `tests/` directory, a
//! module declared under `#[cfg(test)]`, and any item under `#[cfg(test)]` or
//! a test attribute. A sleep there passes inside a test whose time is paused,
//! by `#[tokio::test(start_paused = true)]` or an earlier
//! `tokio::time::pause()`, or with a `// wall-clock: <why>` comment directly
//! above it, for a real subprocess or network peer with no event to await.
//! Generated files, marked `linguist-generated` in `.gitattributes`, are
//! skipped.

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use proc_macro2::{Delimiter, Spacing, Span, TokenStream, TokenTree};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, Lit, Meta, Token};

use crate::CommandError;
use crate::workspace::Workspace;

/// Why `test-waits` failed: it found sleeps in tests, or couldn't look.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Test code sleeps without saying why.
    #[error(
        "{} sleeps in tests:{}\ntests wait for events, not time (docs/dev/rust-conventions.md): \
         await the event that marks the milestone, move time with \
         #[tokio::test(start_paused = true)], or say why on the line above with \
         `// wall-clock: <why>` for a real subprocess or network peer",
        .0.len(),
        Waits(.0)
    )]
    Waits(Vec<Wait>),
    /// A Rust file couldn't be read.
    #[error("read {}", path.display())]
    Read {
        /// The file, from the repository's root.
        path: PathBuf,
        /// Why it couldn't be read.
        #[source]
        source: std::io::Error,
    },
    /// A Rust file couldn't be parsed.
    #[error("parse {}:{line}:{column}: {message}", path.display())]
    Parse {
        /// The file, from the repository's root.
        path: PathBuf,
        /// The line where parsing failed, from 1.
        line: usize,
        /// The column where parsing failed, from 0.
        column: usize,
        /// Why parsing failed.
        message: String,
    },
    /// git couldn't list the repository's Rust files.
    #[error(transparent)]
    Command(#[from] CommandError),
}

/// Sleeps as an error lists them, one per line.
struct Waits<'a>(&'a [Wait]);

impl fmt::Display for Waits<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|wait| write!(f, "\n  {wait}"))
    }
}

/// Checks the Rust files git tracks or would track under `root`, the
/// repository's root, for sleeps in tests.
///
/// # Errors
///
/// [`Error::Waits`] listing every sleep in test code that neither runs on
/// paused time nor says why, or the error that kept a file from being read.
pub fn run(root: &Path) -> Result<(), Error> {
    let files = Workspace::new(root).output(
        "git",
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            ":(attr:!linguist-generated)*.rs",
        ],
    )?;
    let sources = files
        .lines()
        .map(|path| {
            let text = std::fs::read_to_string(root.join(path)).map_err(|source| Error::Read {
                path: PathBuf::from(path),
                source,
            })?;
            Source::parse(PathBuf::from(path), text)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut waits = Source::test_waits(&sources);
    waits.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
    if waits.is_empty() {
        Ok(())
    } else {
        Err(Error::Waits(waits))
    }
}

/// A function that moves or waits on tokio's or the system's clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Clock {
    ThreadSleep,
    Sleep,
    SleepUntil,
    /// `tokio::time::pause`, after which the test's tokio sleeps are instant.
    Pause,
}

impl Clock {
    const ALL: [Self; 4] = [
        Self::ThreadSleep,
        Self::Sleep,
        Self::SleepUntil,
        Self::Pause,
    ];

    fn path(self) -> &'static [&'static str] {
        match self {
            Self::ThreadSleep => &["std", "thread", "sleep"],
            Self::Sleep => &["tokio", "time", "sleep"],
            Self::SleepUntil => &["tokio", "time", "sleep_until"],
            Self::Pause => &["tokio", "time", "pause"],
        }
    }

    /// The function a fully qualified path names, if it is one of these.
    fn named(path: &[String]) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|clock| clock.path().iter().eq(path.iter()))
    }

    /// Whether paused tokio time makes this wait instant.
    fn tokio(self) -> bool {
        matches!(self, Self::Sleep | Self::SleepUntil)
    }
}

impl fmt::Display for Clock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path().join("::"))
    }
}

/// The names a file's `use` items bring in, from any scope of the file.
#[derive(Default)]
struct Imports {
    /// Each imported name or alias and the path it stands for.
    names: HashMap<String, Vec<String>>,
    /// The modules glob-imported with `*`.
    globs: Vec<Vec<String>>,
}

impl Imports {
    fn of(file: &syn::File) -> Self {
        let mut imports = Self::default();
        imports.visit_file(file);
        imports
    }

    fn add(&mut self, mut prefix: Vec<String>, tree: &syn::UseTree) {
        match tree {
            syn::UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.add(prefix, &path.tree);
            }
            syn::UseTree::Name(name) if name.ident == "self" => {
                if let Some(last) = prefix.last() {
                    self.names.insert(last.clone(), prefix);
                }
            }
            syn::UseTree::Name(name) => {
                prefix.push(name.ident.to_string());
                self.names.insert(name.ident.to_string(), prefix);
            }
            syn::UseTree::Rename(rename) => {
                if rename.ident != "self" {
                    prefix.push(rename.ident.to_string());
                }
                self.names.insert(rename.rename.to_string(), prefix);
            }
            syn::UseTree::Glob(_) => self.globs.push(prefix),
            syn::UseTree::Group(group) => {
                for tree in &group.items {
                    self.add(prefix.clone(), tree);
                }
            }
        }
    }

    /// The clock function `path` calls, through this file's imports and,
    /// for `use super::*`, its parent module's.
    fn resolve(&self, path: &[String], parent: Option<&Self>) -> Option<Clock> {
        let (first, rest) = path.split_first()?;
        if let Some(full) = self.names.get(first) {
            return Clock::named(&[full.as_slice(), rest].concat());
        }
        Clock::named(path).or_else(|| {
            self.globs
                .iter()
                .find_map(|glob| match (glob.as_slice(), parent) {
                    ([only], Some(parent)) if only == "super" => parent.resolve(path, None),
                    _ => Clock::named(&[glob.as_slice(), path].concat()),
                })
        })
    }
}

impl<'ast> Visit<'ast> for Imports {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.add(Vec::new(), &item.tree);
    }
}

/// One parsed Rust file of the workspace.
struct Source {
    /// Relative to the workspace root.
    path: PathBuf,
    text: String,
    syntax: syn::File,
}

impl Source {
    fn parse(path: PathBuf, text: String) -> Result<Self, Error> {
        match syn::parse_file(&text) {
            Ok(syntax) => Ok(Self { path, text, syntax }),
            Err(error) => {
                let start = error.span().start();
                Err(Error::Parse {
                    path,
                    line: start.line,
                    column: start.column,
                    message: error.to_string(),
                })
            }
        }
    }

    /// The sleeps in the test code of `sources`, by file and line.
    fn test_waits(sources: &[Self]) -> Vec<Wait> {
        let imports: Vec<Imports> = sources
            .iter()
            .map(|source| Imports::of(&source.syntax))
            .collect();
        let modules: HashMap<PathBuf, &Imports> =
            sources.iter().map(Self::module_dir).zip(&imports).collect();
        let scans: Vec<Scan> = sources
            .iter()
            .zip(&imports)
            .map(|(source, imports)| {
                let parent = source
                    .parent_dir()
                    .and_then(|dir| modules.get(&dir).copied());
                Scan::of(source, imports, parent)
            })
            .collect();
        let test_modules: Vec<&PathBuf> =
            scans.iter().flat_map(|scan| &scan.test_modules).collect();
        sources
            .iter()
            .zip(&scans)
            .flat_map(|(source, scan)| {
                let test_file =
                    source.is_test_file() || test_modules.iter().any(|module| source.is_in(module));
                scan.waits
                    .iter()
                    .filter(move |wait| test_file || wait.test)
                    .map(|wait| Wait {
                        path: source.path.clone(),
                        line: wait.line,
                        clock: wait.clock,
                    })
            })
            .collect()
    }

    /// Whether the file holds only tests: `tests.rs`, or under `tests/`.
    fn is_test_file(&self) -> bool {
        self.path.file_name().is_some_and(|name| name == "tests.rs")
            || self
                .path
                .parent()
                .is_some_and(|dir| dir.iter().any(|part| part == "tests"))
    }

    /// Whether the file is the module `module` (a path without extension) or
    /// one of its children.
    fn is_in(&self, module: &Path) -> bool {
        self.path == module.with_extension("rs") || self.path.starts_with(module)
    }

    /// The directory holding the files of this file's child modules.
    fn module_dir(&self) -> PathBuf {
        let dir = self.path.parent().unwrap_or(Path::new("")).to_owned();
        match self.path.file_stem().and_then(|stem| stem.to_str()) {
            Some("mod" | "lib" | "main") | None => dir,
            Some(stem) => dir.join(stem),
        }
    }

    /// The `module_dir` of the module this file is a child of, or `None` for
    /// a crate root.
    fn parent_dir(&self) -> Option<PathBuf> {
        let dir = self.path.parent()?;
        match self.path.file_stem()?.to_str()? {
            "lib" | "main" => None,
            "mod" => dir.parent().map(Path::to_owned),
            _ => Some(dir.to_owned()),
        }
    }

    /// Whether the comment block directly above `line` (1-based) holds a
    /// `// wall-clock: <why>` with a reason.
    fn explains(&self, line: usize) -> bool {
        self.text
            .lines()
            .take(line.saturating_sub(1))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map_while(|above| above.trim().strip_prefix("//"))
            .any(|comment| {
                comment
                    .trim()
                    .strip_prefix("wall-clock:")
                    .is_some_and(|why| !why.trim().is_empty())
            })
    }
}

/// A sleep in test code, printed as `path:line: <function> in a test`.
#[derive(Debug, PartialEq, Eq)]
pub struct Wait {
    /// The file, from the repository's root.
    path: PathBuf,
    /// Its line, from 1.
    line: usize,
    clock: Clock,
}

impl fmt::Display for Wait {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {} in a test",
            self.path.display(),
            self.line,
            self.clock
        )
    }
}

/// A sleep found in one file, before knowing whether the whole file is test
/// code.
struct Found {
    line: usize,
    clock: Clock,
    /// Under `#[cfg(test)]` or a test attribute within the file.
    test: bool,
}

/// Walks one file, tracking whether it is in test code and whether the
/// current test paused time.
struct Scan<'a> {
    source: &'a Source,
    imports: &'a Imports,
    /// The parent module's imports, which `use super::*` brings in.
    parent: Option<&'a Imports>,
    /// Where the files of the current module's children live.
    module_dir: PathBuf,
    test: bool,
    paused: bool,
    /// Modules declared under `#[cfg(test)]` in their own files, as paths
    /// without extension.
    test_modules: Vec<PathBuf>,
    /// Sleeps neither paused nor explained by a comment.
    waits: Vec<Found>,
}

impl<'a> Scan<'a> {
    fn of(source: &'a Source, imports: &'a Imports, parent: Option<&'a Imports>) -> Self {
        let mut scan = Self {
            source,
            imports,
            parent,
            module_dir: source.module_dir(),
            test: false,
            paused: false,
            test_modules: Vec::new(),
            waits: Vec::new(),
        };
        scan.visit_file(&source.syntax);
        scan
    }

    /// Visits an item with these attributes, which may make it test code or
    /// a test with paused time.
    fn item(&mut self, attrs: &[Attribute], visit: impl FnOnce(&mut Self)) {
        let (test, paused) = (self.test, self.paused);
        self.test |= attrs
            .iter()
            .any(|attr| Self::is_cfg_test(attr) || Self::is_test(attr));
        self.paused |= attrs.iter().any(Self::starts_paused);
        visit(self);
        (self.test, self.paused) = (test, paused);
    }

    /// Records a call to `path`, at `span`.
    fn call(&mut self, path: &[String], span: Span) {
        let Some(clock) = self.imports.resolve(path, self.parent) else {
            return;
        };
        let line = span.start().line;
        if clock == Clock::Pause {
            self.paused = true;
            return;
        }
        let allowed = (self.paused && clock.tokio()) || self.source.explains(line);
        if !allowed {
            self.waits.push(Found {
                line,
                clock,
                test: self.test,
            });
        }
    }

    /// Finds calls in a macro's tokens, which syn leaves unparsed: a path
    /// followed by parentheses.
    fn tokens(&mut self, tokens: TokenStream) {
        let mut path: Vec<String> = Vec::new();
        let mut start = Span::call_site();
        // After a `::`, the next name continues the path.
        let mut joined = false;
        for token in tokens {
            match token {
                TokenTree::Ident(ident) => {
                    if !joined {
                        path.clear();
                        start = ident.span();
                    }
                    path.push(ident.to_string());
                    joined = false;
                }
                // The first colon of `::` is joint; a lone colon, as in
                // `field: value`, ends the path.
                TokenTree::Punct(punct)
                    if punct.as_char() == ':' && (punct.spacing() == Spacing::Joint || joined) =>
                {
                    joined = true;
                }
                TokenTree::Group(group) => {
                    if group.delimiter() == Delimiter::Parenthesis && !path.is_empty() {
                        self.call(&path, start);
                    }
                    self.tokens(group.stream());
                    path.clear();
                    joined = false;
                }
                TokenTree::Punct(_) | TokenTree::Literal(_) => {
                    path.clear();
                    joined = false;
                }
            }
        }
    }

    /// `#[cfg(...)]` whose predicate holds only in tests.
    fn is_cfg_test(attr: &Attribute) -> bool {
        attr.path().is_ident("cfg")
            && attr
                .parse_args::<Meta>()
                .is_ok_and(|meta| Self::implies_test(&meta))
    }

    /// Whether a `cfg` predicate holds only when `test` does.
    fn implies_test(meta: &Meta) -> bool {
        let Meta::List(list) = meta else {
            return meta.path().is_ident("test");
        };
        let Ok(predicates) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        else {
            return false;
        };
        if list.path.is_ident("all") {
            predicates.iter().any(Self::implies_test)
        } else if list.path.is_ident("any") {
            !predicates.is_empty() && predicates.iter().all(Self::implies_test)
        } else {
            false
        }
    }

    /// A test attribute, such as `#[test]` or `#[tokio::test]`.
    fn is_test(attr: &Attribute) -> bool {
        attr.path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "test" || segment.ident == "wasm_bindgen_test")
    }

    /// `#[tokio::test(start_paused = true)]`.
    fn starts_paused(attr: &Attribute) -> bool {
        Self::is_test(attr)
            && attr
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .is_ok_and(|args| {
                    args.iter().any(|arg| {
                        matches!(arg, Meta::NameValue(pair)
                            if pair.path.is_ident("start_paused")
                                && matches!(&pair.value, Expr::Lit(lit)
                                    if matches!(&lit.lit, Lit::Bool(value) if value.value)))
                    })
                })
    }
}

impl<'ast> Visit<'ast> for Scan<'_> {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        self.item(&item.attrs, |scan| {
            let module = scan.module_dir.join(item.ident.to_string());
            if item.content.is_none() {
                if scan.test {
                    scan.test_modules.push(module);
                }
                return;
            }
            let parent = std::mem::replace(&mut scan.module_dir, module);
            visit::visit_item_mod(scan, item);
            scan.module_dir = parent;
        });
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.item(&item.attrs, |scan| visit::visit_item_fn(scan, item));
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.item(&item.attrs, |scan| visit::visit_impl_item_fn(scan, item));
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        self.item(&item.attrs, |scan| visit::visit_trait_item_fn(scan, item));
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        self.item(&item.attrs, |scan| visit::visit_item_impl(scan, item));
    }

    fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
        self.item(&item.attrs, |scan| visit::visit_item_trait(scan, item));
    }

    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        self.item(&item.attrs, |scan| visit::visit_item_const(scan, item));
    }

    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        self.item(&item.attrs, |scan| visit::visit_item_static(scan, item));
    }

    fn visit_item_macro(&mut self, item: &'ast syn::ItemMacro) {
        self.item(&item.attrs, |scan| visit::visit_item_macro(scan, item));
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let Expr::Path(path) = &*call.func {
            let segments: Vec<String> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            self.call(&segments, path.span());
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.tokens(mac.tokens.clone());
        visit::visit_macro(self, mac);
    }
}
