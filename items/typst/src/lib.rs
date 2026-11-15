//! typst for Kuria (`pdf.packages`): a `World` with typst's bundled fonts
//! and Kuria's bundled packages, reading no network and no file system,
//! only the files it is given. It renders PDFs (`pdf.render`) and diagram
//! blocks (`akt.field.markdown.diagrams`).

use std::collections::HashMap;
use std::sync::OnceLock;

use typst::diag::{FileError, FileResult, PackageError, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime};
use typst::layout::{Abs, PagedDocument};
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

include!(concat!(env!("OUT_DIR"), "/packages.rs"));

/// What a diagram block has imported for it, and every PDF too, so that a
/// diagram drawn in a PDF reads as it does on its own.
pub const IMPORTS: &str = "#import \"@preview/fletcher:0.5.8\" as fletcher: diagram, node, edge
#import \"@preview/cetz:0.5.2\" as cetz: canvas, draw
#import \"@preview/cetz-plot:0.1.4\": plot, chart
";

/// A diagram's page: as big as the diagram, with a little room around it.
const DIAGRAM_PAGE: &str = "#set page(width: auto, height: auto, margin: 4pt, fill: none)
#set text(font: \"Libertinus Serif\", size: 11pt)
";

/// typst's fonts, read once.
fn fonts() -> &'static (LazyHash<FontBook>, Vec<Font>) {
    static FONTS: OnceLock<(LazyHash<FontBook>, Vec<Font>)> = OnceLock::new();
    FONTS.get_or_init(|| {
        let fonts: Vec<Font> = typst_assets::fonts()
            .flat_map(|data| Font::iter(Bytes::new(data)))
            .collect();
        (LazyHash::new(FontBook::from_fonts(&fonts)), fonts)
    })
}

/// The bundled packages' files, by package name, version and path.
fn packages() -> &'static HashMap<(&'static str, &'static str, &'static str), Bytes> {
    static PACKAGES: OnceLock<HashMap<(&str, &str, &str), Bytes>> = OnceLock::new();
    PACKAGES.get_or_init(|| {
        PACKAGE_FILES
            .iter()
            .map(|(name, version, path, bytes)| ((*name, *version, *path), Bytes::new(*bytes)))
            .collect()
    })
}

/// One typst document: its source, the bundled packages, and the files it
/// is given, and nothing else.
struct Doc {
    library: LazyHash<Library>,
    main: Source,
    files: HashMap<FileId, Bytes>,
}

impl Doc {
    fn new(markup: String, files: Vec<(String, Vec<u8>)>) -> Self {
        Self {
            library: LazyHash::new(Library::default()),
            main: Source::new(FileId::new(None, VirtualPath::new("/main.typ")), markup),
            files: files
                .into_iter()
                .map(|(path, bytes)| {
                    (
                        FileId::new(None, VirtualPath::new(&path)),
                        Bytes::new(bytes),
                    )
                })
                .collect(),
        }
    }

    /// A bundled package's file, refusing every other package.
    fn packaged(id: FileId) -> FileResult<Bytes> {
        let Some(spec) = id.package() else {
            return Err(FileError::AccessDenied);
        };
        if spec.namespace != "preview" {
            return Err(FileError::Package(PackageError::NotFound(spec.clone())));
        }
        let version = spec.version.to_string();
        let path = id
            .vpath()
            .as_rootless_path()
            .to_string_lossy()
            .replace('\\', "/");
        packages()
            .iter()
            .find(|((name, v, p), _)| *name == spec.name.as_str() && *v == version && *p == path)
            .map(|(_, bytes)| bytes.clone())
            .ok_or_else(|| {
                if packages()
                    .keys()
                    .any(|(name, v, _)| *name == spec.name.as_str() && *v == version)
                {
                    FileError::NotFound(id.vpath().as_rootless_path().to_owned())
                } else {
                    FileError::Package(PackageError::NotFound(spec.clone()))
                }
            })
    }
}

impl World for Doc {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &fonts().0
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            return Ok(self.main.clone());
        }
        let bytes = Self::packaged(id)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| FileError::InvalidUtf8)?;
        Ok(Source::new(id, text.to_owned()))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id.package().is_some() {
            return Self::packaged(id);
        }
        self.files.get(&id).cloned().ok_or(FileError::AccessDenied)
    }

    fn font(&self, index: usize) -> Option<Font> {
        fonts().1.get(index).cloned()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        None
    }
}

/// typst's errors, as one message.
fn said(errors: &[SourceDiagnostic]) -> String {
    errors
        .iter()
        .map(|e| e.message.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

fn compile(doc: &Doc) -> Result<PagedDocument, String> {
    typst::compile(doc).output.map_err(|e| said(&e))
}

/// `markup`, typst, as a PDF, reading only `files`, by path, and the
/// bundled packages.
// r[impl pdf.packages]
// r[impl pdf.repeatable]
pub fn pdf(markup: String, files: Vec<(String, Vec<u8>)>) -> Result<Vec<u8>, String> {
    let document = compile(&Doc::new(markup, files))?;
    typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).map_err(|e| said(&e))
}

/// A diagram block's `source`, with the bundled packages imported, as SVG
/// sized to the diagram; typst's message if it doesn't compile
/// (`akt.field.markdown.diagrams`).
// r[impl akt.field.markdown.diagrams]
// r[impl pdf.packages]
pub fn diagram(source: &str) -> Result<String, String> {
    let markup = format!("{DIAGRAM_PAGE}{IMPORTS}{source}");
    let document = compile(&Doc::new(markup, Vec::new()))?;
    if document.pages.is_empty() {
        return Err("the diagram draws nothing".to_owned());
    }
    Ok(typst_svg::svg_merged(&document, Abs::zero()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify pdf.packages]
    #[test]
    fn every_bundled_package_is_imported() {
        let svg = diagram("#text[Imported]").unwrap();
        assert!(svg.starts_with("<svg"), "{svg}");
    }

    // r[verify akt.field.markdown.diagrams]
    #[test]
    fn a_fletcher_diagram_and_a_plot_render() {
        let arrows =
            diagram("#diagram(node((0, 0), [Write]), edge(\"-|>\"), node((1, 0), [Release]))")
                .unwrap();
        assert!(
            arrows.starts_with("<svg") && arrows.contains("<path"),
            "{arrows}"
        );
        let plotted =
            diagram("#canvas({ plot.plot(size: (4, 3), { plot.add(((0, 0), (1, 1), (2, 4))) }) })")
                .unwrap();
        assert!(plotted.starts_with("<svg"), "{plotted}");
        // The same source renders the same way each time.
        assert_eq!(
            diagram("#diagram(node((0, 0), [A]))").unwrap(),
            diagram("#diagram(node((0, 0), [A]))").unwrap()
        );
    }

    // r[verify pdf.packages]
    #[test]
    fn nothing_else_can_be_read() {
        for (source, says) in [
            ("#import \"@preview/tablex:0.0.8\": tablex", "tablex"),
            ("#import \"@local/mine:0.1.0\": x", "mine"),
            ("#read(\"/etc/passwd\")", "access denied"),
            ("#image(\"https://example.com/a.png\")", ""),
            ("#diagram(", ""),
        ] {
            let refused = diagram(source).unwrap_err();
            assert!(refused.to_lowercase().contains(says), "{source}: {refused}");
        }
    }

    #[test]
    fn a_pdf_reads_only_what_it_is_given() {
        let made = pdf(format!("{IMPORTS}#diagram(node((0, 0), [A]))"), Vec::new()).unwrap();
        assert!(made.starts_with(b"%PDF"));
        assert!(pdf("#read(\"/etc/hosts\")".to_owned(), Vec::new()).is_err());
    }
}
