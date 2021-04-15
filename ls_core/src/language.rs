//! Support for intelligent parsing / understanding of source code


extern crate test;

use tree_sitter;
use tree_sitter_rust;
use tree_sitter_cpp;
use tree_sitter_java;
use tree_sitter_javascript;
use tree_sitter_python;
use tree_sitter_typescript;
use tree_sitter_bash;
use lazy_static::lazy_static;

use crate::document;

extern "C" { fn tree_sitter_test() -> tree_sitter::Language; }

lazy_static! {
    static ref LANGUAGES: Vec<(&'static str, tree_sitter::Language)> = vec![
        ("rs", tree_sitter_rust::language()),
        ("cpp", tree_sitter_cpp::language()),
        ("java", tree_sitter_java::language()),
        ("js", tree_sitter_javascript::language()),
        ("py", tree_sitter_python::language()),
        ("ts", tree_sitter_typescript::language_typescript()),
        ("tsx", tree_sitter_typescript::language_tsx()),
        ("sh", tree_sitter_bash::language()),
        ("test", unsafe { tree_sitter_test() })
    ];
}

pub fn get_parser(lang_str: &str) -> Option<tree_sitter::Parser> {
    for (name, lang) in LANGUAGES.iter() {
        if name == &lang_str {
            let mut parser = tree_sitter::Parser::new();
            parser.set_language(*lang).ok()?;
            return Some(parser);
        }
    }

    None
}

fn pp_rec(node: &tree_sitter::Node, out: String, depth: i32, doc: &document::Document) -> String {
    let mut result = out;

    for _ in 0..depth {
        result += "   ";
    }

    result += node.kind();
    
    let range = node.range();
    result += &format!(" ({}.{} - {}.{})", 
        range.start_point.row, range.start_point.column,
        range.end_point.row, range.end_point.column
    );

    if range.end_point.row == range.start_point.row {
        let line = doc.line(range.start_point.row).unwrap();
        result += &format!(" \"{}\"", 
            line[range.start_point.column..range.end_point.column].to_string());
    }

    result += "\n";
    for i in 0..node.child_count() {
        result = pp_rec(&node.child(i).unwrap(), result, depth + 1, doc);
    }

    result
}

pub fn pretty_print(node: &tree_sitter::Node, doc: &document::Document) -> String {
    pp_rec(node, String::new(), 0i32, doc)
}











#[cfg(test)]
mod tests {
    use super::*;
    use test::Bencher;

    #[test]
    fn test_custom_parser() {
        let doc = document::Document::from_with_language(
r#"
language Rust {
    extension: "rs";
    casing: "snake";
    raw: false;
    annoying: 1;
}
"#, "test");

        assert_eq!(
            format!("{}", doc.get_context_at(&document::Position::from(3, 15)).unwrap()),
r#"source_file (1, 0)-(7, 0)
language (1, 0)-(6, 1)
pair (3, 4)-(3, 20)
literal (3, 12)-(3, 19)
string_literal (3, 12)-(3, 19)
string_content (3, 13)-(3, 18)
"#);
    }

    #[bench]
    fn bench_doc_create(b: &mut Bencher) {
        b.iter(|| {
            let doc = document::Document::from(TESTCODE);
            test::black_box(&doc);
        });
    }

    #[bench]
    fn bench_doc_text(b: &mut Bencher) {
        let doc = document::Document::from(TESTCODE);

        b.iter(|| {
            test::black_box(&doc.text());
        });
    }

    #[bench]
    fn bench_ts_pprint(b: &mut Bencher) {
        let mut parser = get_parser("rs").unwrap();
        let doc = document::Document::from(TESTCODE);
        let tree = parser.parse(doc.text(), None).unwrap();

        b.iter(|| {
            test::black_box(&pretty_print(&tree.root_node(), &doc));
        });
    }

    #[bench]
    fn bench_ts_parse(b: &mut Bencher) {
        let mut parser = get_parser("rs").unwrap();
        let doc = document::Document::from(TESTCODE);
    
        b.iter(|| {
            let tree = parser.parse(doc.text(), None).unwrap();
            test::black_box(&tree);
        });
        
        //println!("{}", language::pretty_print(&tree.root_node(), &doc));
    }

    fn insert_times(n: usize) {
        let mut doc = document::Document::from_with_language("", "rs");
        doc.insert("fn test() {\n\n}\n", &document::InsertOptions::exact()).unwrap();
        doc.set_cursor_and_mark(&document::Position::from(1, 0)).unwrap();
        for i in 0..n {
            doc.insert("    let x = 10;\n", &document::InsertOptions::exact()).unwrap();
        }
    }

    #[bench]
    fn bench_insert_010(b: &mut Bencher) {
        b.iter(|| {
            insert_times(10);
        });
    }

    #[bench]
    fn bench_insert_020(b: &mut Bencher) {
        b.iter(|| {
            insert_times(20);
        });
    }

    #[bench]
    fn bench_insert_050(b: &mut Bencher) {
        b.iter(|| {
            insert_times(50);
        });
    }

    #[bench]
    fn bench_insert_100(b: &mut Bencher) {
        b.iter(|| {
            insert_times(100);
        });
    }

    #[bench]
    fn bench_insert_200(b: &mut Bencher) {
        b.iter(|| {
            insert_times(200);
        });
    }

    const TESTCODE: &str = r#"/// Sets anchor `handle` to `value`. Returns an `Err` if `handle` does not
/// exist or if `value` points to an invalid position.
pub fn set_anchor(&mut self, handle: AnchorHandle, value: &Anchor) -> Result<(), Oops> {
    if let None = self.anchors.get(handle) {
        return Err(Oops::NonexistentAnchor(handle));
    }
    if !self.position_valid(&value.position) {
        return Err(Oops::InvalidPosition(value.position, "set_anchor"));
    }

    let inverse = self.set_anchor_untracked(handle, value);
    self.undo_redo.push_undo(inverse);

    Ok(())
}

/// Creates a new anchor with contents `anchor`, returning its
/// [`AnchorHandle`] or `Err` if the requested position is invalid.
pub fn create_anchor(&mut self, anchor: &Anchor) -> Result<AnchorHandle, Oops> {
    if !self.position_valid(&anchor.position) {
        return Err(Oops::InvalidPosition(anchor.position, "create_anchor"));
    }

    let handle = self.anchors.get_new_handle();
    let inverse = self.insert_anchor_untracked(handle, anchor);
    self.undo_redo.push_undo(inverse);

    Ok(handle)
}

/// Moves the cursor to `position`.
pub fn set_cursor(&mut self, position: &Position) -> Result<(), Oops> {
    self.set_anchor(Anchors::CURSOR, &Anchor {
        position: *position,
        ..*self.anchors.get(Anchors::CURSOR).unwrap()
    })
}

/// Moves the mark to `position`.
pub fn set_mark(&mut self, position: &Position) -> Result<(), Oops> {
    self.set_anchor(Anchors::MARK, &Anchor {
        position: *position,
        ..*self.anchors.get(Anchors::MARK).unwrap()
    })
}

/// Moves both cursor and mark to `position`.
pub fn set_cursor_and_mark(&mut self, position: &Position) -> Result<(), Oops> {
    self.set_cursor(position)?;
    self.set_mark(position)?;
    Ok(())
}

/// Moves the mark to the beginning of `range` and the cursor to the 
/// end of `range`.
pub fn set_selection(&mut self, range: &Range) -> Result<(), Oops> {
    if !self.range_valid(range) {
        Err(Oops::InvalidRange(*range, "set_selection"))
    } else {
        self.set_mark(&range.beginning)?;
        self.set_cursor(&range.ending)?;
        Ok(())
    }
}

/// Removes the anchor at `handle`, or returns `Err` if invalid.
pub fn remove_anchor(&mut self, handle: AnchorHandle) -> Result<(), Oops> {
    if let None = self.anchors.get(handle) {
        return Err(Oops::NonexistentAnchor(handle));
    }

    let inverse = self.remove_anchor_untracked(handle);

    self.undo_redo.push_undo(inverse);
    Ok(())
}

/// Sets the indentation policy of this document to `indentation`.
/// Does not actually change the document's text!
pub fn set_indentation(&mut self, indentation: &Indentation) -> Result<(), Oops> {
    let inverse = self.set_indentation_untracked(indentation);
    self.undo_redo.push_undo(inverse);
    Ok(())
}


/// Undoes the most recently performed [`ChangePacket`], or returns error
/// if there is nothing to undo.
pub fn undo_once(&mut self) -> Result<(), Oops> {
    match self.undo_redo.undo_stack.pop() {
        None => Err(Oops::NoMoreUndos(0)),
        Some(packet) => {
            let mut redo_packet = ChangePacket::new();
            for inverse in packet.changes.iter().rev() {
                redo_packet.changes.push(inverse.apply_untracked(self));
            }
            
            self.undo_redo.redo_stack.push(redo_packet);
            Ok(())
        }
    }
}
"#;
}


