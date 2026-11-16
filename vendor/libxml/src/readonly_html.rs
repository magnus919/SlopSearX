//! Affine read-only HTML document candidate. No mutable tree wrapper escapes.
#![deny(unsafe_op_in_unsafe_fn)]
use crate::{bindings as c, budgeted::ChildAllocator};
use ssx_owned_budget::OwnedVec;
use std::{
  ffi::{CStr, c_void},
  marker::PhantomData,
  ptr::{self, NonNull},
  rc::Rc,
};
/// Closed parse/getter failure; caller keeps source declaration checks separately.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
  /// Allocation refusal, poisoned root or sticky C OOM; never publish partial data.
  Resource,
  /// Input contains a value above U+10FFFF.
  InvalidCodepoint,
  /// Checked native input representation exceeds the signed C length bound.
  InputBound,
  /// libxml returned no document without a resource failure.
  Empty,
  /// Startup token/root invariant was not established.
  Invariant,
}
struct Context(NonNull<c::xmlParserCtxt>);
impl Context {
  fn new() -> Result<Self, Error> {
    let raw = unsafe { c::htmlNewParserCtxt() };
    let context = Self(NonNull::new(raw).ok_or(Error::Resource)?);
    unsafe {
      c::xmlCtxtSetResourceLoader(
        context.0.as_ptr(),
        Some(crate::parser::refuse_external_resource),
        ptr::null_mut(),
      );
      c::xmlCtxtSetErrorHandler(
        context.0.as_ptr(),
        Some(discard_diagnostic),
        ptr::null_mut(),
      );
    }
    Ok(context)
  }
}
impl Drop for Context {
  fn drop(&mut self) {
    unsafe {
      c::htmlFreeParserCtxt(self.0.as_ptr());
    }
  }
}
unsafe extern "C" fn discard_diagnostic(_context: *mut c_void, _error: *const c::xmlError) {}
/// Immutable document. Its allocator and C storage outlive every borrowed view.
/// Neither document nor views can be sent/shared across threads or cloned as owners.
///
/// ```compile_fail
/// use libxml::{budgeted::ChildAllocator,readonly_html::{Document,Node}};
/// fn escape(allocator:&ChildAllocator)->Node<'_,'_> {
///   let document=Document::parse(allocator,&[60,112,62,120]).unwrap();
///   document.root().unwrap()
/// }
/// ```
/// ```compile_fail
/// use libxml::readonly_html::Document;
/// fn send<T:Send>() {}
/// send::<Document<'static>>();
/// ```
pub struct Document<'child> {
  raw: NonNull<c::xmlDoc>,
  allocator: &'child ChildAllocator,
  _local: PhantomData<Rc<()>>,
}
impl<'child> Document<'child> {
  /// Parse source codepoints using the source minimal PEP393 width and fixed
  /// native-endian encoding. No file/URL/options/encoding selector is accepted.
  pub fn parse(allocator: &'child ChildAllocator, points: &[u32]) -> Result<Self, Error> {
    if allocator.failed() {
      return Err(Error::Resource);
    }
    let budget = allocator.root_budget().ok_or(Error::Invariant)?;
    let mut maximum = 0;
    for &point in points {
      if point > 0x10ffff {
        return Err(Error::InvalidCodepoint);
      }
      maximum = maximum.max(point);
    }
    let width = if maximum <= 255 {
      1
    } else if maximum <= 65535 {
      2
    } else {
      4
    };
    let length = points
      .len()
      .checked_mul(width)
      .filter(|n| *n <= i32::MAX as usize)
      .ok_or(Error::InputBound)?;
    let mut bytes = OwnedVec::with_capacity(budget, length).map_err(|_| Error::Resource)?;
    for point in points {
      match width {
        1 => bytes.try_push(*point as u8),
        2 => bytes.try_extend_from_slice(&(*point as u16).to_ne_bytes()),
        _ => bytes.try_extend_from_slice(&point.to_ne_bytes()),
      }
      .map_err(|_| Error::Resource)?;
    }
    let encoding = if maximum <= 127 {
      c"UTF-8"
    } else if maximum <= 255 {
      c"ISO-8859-1"
    } else if width == 2 {
      if cfg!(target_endian = "little") {
        c"UTF-16LE"
      } else {
        c"UTF-16BE"
      }
    } else if cfg!(target_endian = "little") {
      c"UTF-32LE"
    } else {
      c"UTF-32BE"
    };
    // SAFETY: startup token established the process-lifetime callback root. The
    // unique C context remains owned until read completes and is freed on all paths.
    let context = Context::new()?;
    let options = c::htmlParserOption_HTML_PARSE_RECOVER
      | c::htmlParserOption_HTML_PARSE_NOERROR
      | c::htmlParserOption_HTML_PARSE_NOWARNING
      | c::htmlParserOption_HTML_PARSE_NONET
      | c::htmlParserOption_HTML_PARSE_COMPACT;
    // SAFETY: checked <=INT_MAX, borrowed bytes live through synchronous parse;
    // fixed static encoding and null base URL cannot invoke a caller path/loader.
    #[cfg(test)]
    tests::ENTERED.store(true, std::sync::atomic::Ordering::SeqCst);
    let raw = unsafe {
      c::htmlCtxtReadMemory(
        context.0.as_ptr(),
        bytes.as_slice().as_ptr().cast(),
        length as i32,
        ptr::null(),
        encoding.as_ptr(),
        options as i32,
      )
    };
    #[cfg(test)]
    tests::NONNULL.store(!raw.is_null(), std::sync::atomic::Ordering::SeqCst);
    drop(context);
    drop(bytes);
    publish_document(raw, allocator)
  }

  /// First element root, skipping document-level DTD/comment nodes through C API.
  pub fn root(&self) -> Option<Node<'_, 'child>> {
    let raw = unsafe { c::xmlDocGetRootElement(self.raw.as_ptr()) };
    NonNull::new(raw).map(|raw| Node {
      raw,
      document: self,
    })
  }
}
fn publish_document<'a>(
  raw: *mut c::xmlDoc,
  allocator: &'a ChildAllocator,
) -> Result<Document<'a>, Error> {
  let document = NonNull::new(raw).map(|raw| Document {
    raw,
    allocator,
    _local: PhantomData,
  });
  if allocator.failed() {
    drop(document);
    return Err(Error::Resource);
  }
  document.ok_or(Error::Empty)
}
impl Drop for Document<'_> {
  fn drop(&mut self) {
    unsafe {
      c::xmlFreeDoc(self.raw.as_ptr());
    }
  }
}
/// A borrowed immutable node. No owning/mutable wrapper or raw pointer is exposed.
#[derive(Clone, Copy)]
pub struct Node<'doc, 'child> {
  raw: NonNull<c::xmlNode>,
  document: &'doc Document<'child>,
}
/// Kind needed by charged flat-tree projection; other nodes are skipped by caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  /// HTML element.
  Element,
  /// Text node.
  Text,
  /// CDATA section.
  Cdata,
  /// Comment, PI or other unsupported node.
  Other,
}
// Read C-owned NUL strings without allocating. Caller proves live tree storage.
unsafe fn c_bytes<'a>(value: *const c::xmlChar) -> &'a [u8] {
  if value.is_null() {
    &[]
  } else {
    unsafe { CStr::from_ptr(value.cast()).to_bytes() }
  }
}
impl<'doc, 'child> Node<'doc, 'child> {
  /// Node classification; never reinterprets element sort indices as text.
  pub fn kind(self) -> Kind {
    let kind = unsafe { ptr::read(ptr::addr_of!((*self.raw.as_ptr()).type_)) };
    match kind {
      c::xmlElementType_XML_ELEMENT_NODE => Kind::Element,
      c::xmlElementType_XML_TEXT_NODE => Kind::Text,
      c::xmlElementType_XML_CDATA_SECTION_NODE => Kind::Cdata,
      _ => Kind::Other,
    }
  }
  /// C-owned local name bytes, with no allocating getter or lossy conversion.
  pub fn name_bytes(self) -> &'doc [u8] {
    let name = unsafe { ptr::read(ptr::addr_of!((*self.raw.as_ptr()).name)) };
    unsafe { c_bytes(name) }
  }
  /// Borrowed content only for actual text/CDATA nodes. Other C content fields
  /// have different meanings (including compact storage and element sort index).
  pub fn text_bytes(self) -> Option<&'doc [u8]> {
    if !matches!(self.kind(), Kind::Text | Kind::Cdata) {
      return None;
    }
    let content = unsafe { ptr::read(ptr::addr_of!((*self.raw.as_ptr()).content)) };
    Some(unsafe { c_bytes(content) })
  }
  /// First child; entity-reference children are not exposed as tree edges.
  pub fn first_child(self) -> Option<Self> {
    if self.kind() != Kind::Element {
      return None;
    }
    let raw = unsafe { ptr::read(ptr::addr_of!((*self.raw.as_ptr()).children)) };
    NonNull::new(raw).map(|raw| Self {
      raw,
      document: self.document,
    })
  }
  /// Next sibling via the documented libxml common node prefix.
  pub fn next_sibling(self) -> Option<Self> {
    let raw = unsafe { ptr::read(ptr::addr_of!((*self.raw.as_ptr()).next)) };
    NonNull::new(raw).map(|raw| Self {
      raw,
      document: self.document,
    })
  }
  /// First attribute of an element; compact text properties are never inspected.
  pub fn first_attribute(self) -> Option<Attribute<'doc, 'child>> {
    if self.kind() != Kind::Element {
      return None;
    }
    let raw = unsafe { ptr::read(ptr::addr_of!((*self.raw.as_ptr()).properties)) };
    NonNull::new(raw).map(|raw| Attribute {
      raw,
      document: self.document,
    })
  }
}
/// Borrowed read-only attribute.
#[derive(Clone, Copy)]
pub struct Attribute<'doc, 'child> {
  raw: NonNull<c::xmlAttr>,
  document: &'doc Document<'child>,
}
impl<'doc, 'child> Attribute<'doc, 'child> {
  /// Attribute name without allocation.
  pub fn name_bytes(self) -> &'doc [u8] {
    unsafe { c_bytes((*self.raw.as_ptr()).name) }
  }
  /// Next attribute without allocating a collection.
  pub fn next_attribute(self) -> Option<Self> {
    NonNull::new(unsafe { (*self.raw.as_ptr()).next }).map(|raw| Self {
      raw,
      document: self.document,
    })
  }
  /// Exact libxml attribute content (entity/CDATA semantics) under installed C
  /// hooks. The temporary has an affine owner and is freed before its charge.
  pub fn value(self) -> Result<Content<'doc, 'child>, Error> {
    if self.document.allocator.failed() {
      return Err(Error::Resource);
    }
    let raw = unsafe { c::xmlNodeGetContent(self.raw.as_ptr().cast()) };
    let content = Content {
      raw: NonNull::new(raw),
      _document: self.document,
    };
    if self.document.allocator.failed() {
      drop(content);
      return Err(Error::Resource);
    }
    Ok(content)
  }
}
/// Owned temporary C content. Its hidden header charge is released only on C free.
pub struct Content<'doc, 'child> {
  raw: Option<NonNull<c::xmlChar>>,
  _document: &'doc Document<'child>,
}
impl Content<'_, '_> {
  /// Borrow content bytes; null from a valid empty C getter maps to empty bytes.
  pub fn as_bytes(&self) -> &[u8] {
    self.raw.map_or(&[], |raw| unsafe { c_bytes(raw.as_ptr()) })
  }
}
impl Drop for Content<'_, '_> {
  fn drop(&mut self) {
    if let Some(raw) = self.raw {
      unsafe {
        crate::budgeted::release_owned(raw.as_ptr().cast());
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use ssx_owned_budget::Budget;
  use std::sync::atomic::{AtomicBool, Ordering};
  pub(super) static ENTERED: AtomicBool = AtomicBool::new(false);
  pub(super) static NONNULL: AtomicBool = AtomicBool::new(false);
  macro_rules! not_impl {
    ($ty:ty,$bound:path) => {
      const _: fn() = || {
        trait Ambiguous<A> {
          fn marker() {}
        }
        impl<T: ?Sized> Ambiguous<()> for T {}
        impl<T: ?Sized + $bound> Ambiguous<u8> for T {}
        let _ = <$ty as Ambiguous<_>>::marker;
      };
    };
  }
  not_impl!(Document<'static>, Send);
  not_impl!(Document<'static>, Sync);
  not_impl!(Document<'static>, Clone);
  not_impl!(Node<'static, 'static>, Send);
  not_impl!(Node<'static, 'static>, Sync);
  not_impl!(Attribute<'static, 'static>, Send);
  not_impl!(Attribute<'static, 'static>, Sync);
  not_impl!(Content<'static, 'static>, Send);
  not_impl!(Content<'static, 'static>, Sync);
  not_impl!(Content<'static, 'static>, Clone);
  #[derive(Debug, PartialEq, Eq)]
  struct Flat {
    kind: Kind,
    name: String,
    text: Option<String>,
    attrs: std::collections::BTreeMap<String, String>,
    children: Vec<Flat>,
  }
  fn new_snapshot(node: Node<'_, '_>) -> Flat {
    let mut attrs = std::collections::BTreeMap::new();
    let mut a = node.first_attribute();
    while let Some(attr) = a {
      let value = attr.value().unwrap();
      attrs.insert(
        String::from_utf8_lossy(attr.name_bytes()).into_owned(),
        String::from_utf8_lossy(value.as_bytes()).into_owned(),
      );
      a = attr.next_attribute();
    }
    let mut children = Vec::new();
    let mut child = node.first_child();
    while let Some(n) = child {
      if n.kind() != Kind::Other {
        children.push(new_snapshot(n));
      }
      child = n.next_sibling();
    }
    Flat {
      kind: node.kind(),
      name: if node.kind() == Kind::Element {
        String::from_utf8_lossy(node.name_bytes()).into_owned()
      } else {
        String::new()
      },
      text: node
        .text_bytes()
        .map(|s| String::from_utf8_lossy(s).into_owned()),
      attrs,
      children,
    }
  }
  fn old_snapshot(node: crate::tree::Node) -> Flat {
    let kind = match node.get_type() {
      Some(crate::tree::NodeType::ElementNode) => Kind::Element,
      Some(crate::tree::NodeType::TextNode) => Kind::Text,
      Some(crate::tree::NodeType::CDataSectionNode) => Kind::Cdata,
      _ => Kind::Other,
    };
    let children = node
      .get_child_nodes()
      .into_iter()
      .filter_map(|n| {
        let s = old_snapshot(n);
        (s.kind != Kind::Other).then_some(s)
      })
      .collect();
    Flat {
      kind,
      name: if kind == Kind::Element {
        node.get_name()
      } else {
        String::new()
      },
      text: matches!(kind, Kind::Text | Kind::Cdata).then(|| node.get_content()),
      attrs: if kind == Kind::Element {
        node.get_properties().into_iter().collect()
      } else {
        Default::default()
      },
      children,
    }
  }
  #[test]
  fn isolated_affine_document_contracts() {
    let Some(case) = std::env::var_os("A05_FACADE_CASE") else {
      for case in ["normal", "loader", "oom", "publication_fault", "getter_oom"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
          .args([
            "--exact",
            "readonly_html::tests::isolated_affine_document_contracts",
            "--nocapture",
          ])
          .env("A05_FACADE_CASE", case)
          .output()
          .unwrap();
        print!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        assert!(output.status.success(), "{case}");
      }
      return;
    };
    let root = Budget::new(if case == "oom" {
      64 * 1024
    } else {
      64 * 1024 * 1024
    })
    .unwrap();
    let allocator = ChildAllocator::install(root.clone()).unwrap();
    if case == "normal" {
      for text in [
        "<p a='&amp;&quot;&#x1f600;'>one<b>two</b>three</p>",
        "<table><tr><td x='1'>one<tr><td>two",
        "<html><head><meta charset='cp437'></head><body>Café</body></html>",
        "<meta charset='utf-8'><p>中文",
        "<!DOCTYPE html><!--a--><p>😀",
        "<p>one\0two<svg a='&#0;'>tail",
        "plain text",
        "<a",
        "<!--comment-->",
      ] {
        let mut points = text.chars().map(u32::from).collect::<Vec<_>>();
        for width in 0..3 {
          if width == 1 {
            points.push(0xd800);
          }
          if width == 2 {
            points.push(0x10000);
          }
          let old = crate::parser::Parser::parse_untrusted_html_codepoints(&points);
          let new = Document::parse(&allocator, &points);
          match (old, new) {
            (Ok((old, _)), Ok(new)) => match (old.get_root_element(), new.root()) {
              (Some(old), Some(new)) => {
                assert_eq!(old_snapshot(old), new_snapshot(new), "{text:?}/{width}")
              }
              (None, None) => {}
              _ => panic!("root mismatch"),
            },
            (Err(_), Err(Error::Empty)) => {}
            (_, Err(error)) => panic!("unexpected immutable error {error:?}"),
            (Err(_), Ok(_)) => panic!("new success for old failure"),
          }
        }
      }
      assert!(matches!(
        Document::parse(&allocator, &[0x110000]),
        Err(Error::InvalidCodepoint)
      ));
      assert!(!allocator.failed());
      println!("A05_FACADE normal controls=27");
    } else if case == "loader" {
      let handler = unsafe { *c::__xmlStructuredError() }.map(|f| f as usize);
      let data = unsafe { *c::__xmlStructuredErrorContext() };
      let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
      listener.set_nonblocking(true).unwrap();
      let file = std::env::temp_dir().join(format!("a05-facade-loader-{}", std::process::id()));
      std::fs::write(&file, b"<SECRET>must-not-load</SECRET>").unwrap();
      for url in [
        format!("file://{}", file.display()),
        file.display().to_string(),
        format!("http://{}/no", listener.local_addr().unwrap()),
      ] {
        let context = Context::new().unwrap();
        let before = crate::parser::A05_REFUSALS.load(Ordering::SeqCst);
        let url = std::ffi::CString::new(url).unwrap();
        let raw = unsafe { c::xmlCtxtReadFile(context.0.as_ptr(), url.as_ptr(), ptr::null(), 0) };
        assert!(raw.is_null());
        assert!(crate::parser::A05_REFUSALS.load(Ordering::SeqCst) > before);
      }
      assert!(matches!(listener.accept(),Err(e)if e.kind()==std::io::ErrorKind::WouldBlock));
      std::fs::remove_file(file).unwrap();
      assert_eq!(
        handler,
        unsafe { *c::__xmlStructuredError() }.map(|f| f as usize)
      );
      assert_eq!(data, unsafe { *c::__xmlStructuredErrorContext() });
      println!("A05_FACADE loader actual_refusals=3");
    } else if case == "oom" {
      let points = "<div a='value'>text</div>"
        .repeat(512)
        .chars()
        .map(u32::from)
        .collect::<Vec<_>>();
      assert!(matches!(
        Document::parse(&allocator, &points),
        Err(Error::Resource)
      ));
      assert!(allocator.failed());
      assert!(ENTERED.load(Ordering::SeqCst));
      println!(
        "A05_FACADE oom parse_entered=true raw_nonnull={}",
        NONNULL.load(Ordering::SeqCst)
      );
    } else if case == "publication_fault" {
      let points = "<p a='value'>one<b>two</b></p>"
        .chars()
        .map(u32::from)
        .collect::<Vec<_>>();
      let doc = Document::parse(&allocator, &points).unwrap();
      let raw = doc.raw.as_ptr();
      let before = root.snapshot().unwrap().used;
      let frees = allocator.callback_counts().1;
      std::mem::forget(doc);
      root.fail_closed();
      assert!(matches!(
        publish_document(raw, &allocator),
        Err(Error::Resource)
      ));
      assert!(root.snapshot().unwrap().used < before);
      assert!(allocator.callback_counts().1 > frees);
      println!("A05_FACADE publication_fault nonnull=true injected_after_parse=true");
    } else {
      let points = "<p a='value'>text</p>"
        .chars()
        .map(u32::from)
        .collect::<Vec<_>>();
      let doc = Document::parse(&allocator, &points).unwrap();
      let attr = doc
        .root()
        .unwrap()
        .first_child()
        .unwrap()
        .first_child()
        .unwrap()
        .first_attribute()
        .unwrap();
      let snapshot = root.snapshot().unwrap();
      let held = root.reserve(snapshot.limit - snapshot.used).unwrap();
      assert!(matches!(attr.value(), Err(Error::Resource)));
      assert!(allocator.failed());
      drop(held);
      drop(doc);
      println!("A05_FACADE getter_oom actual_callback=true");
    }
    // All affine values are gone. Final process-local C cleanup retains hooks
    // and root, and no further parser call occurs in this isolated subprocess.
    unsafe {
      c::xmlResetLastError();
      c::xmlCleanupParser();
    }
    assert_eq!(
      root.snapshot().unwrap().used,
      0,
      "C allocations remain after isolated final cleanup"
    );
  }
}
