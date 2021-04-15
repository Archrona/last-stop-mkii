
use ls_core;
use ls_core::document;
use ls_core::document::{Document, InsertOptions};



#[test]
fn test_tree_sitter() {
    //let parser = language::get_parser("rs").unwrap();
    let code = r#"let 兄弟 = vec!["Ken"];"#;
    let mut doc = Document::from(code);

    //let tree = parser.parse(doc.text(), None).unwrap();
    //println!("{}", language::pretty_print(&tree.root_node(), &doc));

    doc.insert(r#", "Chad""#, &InsertOptions::exact_at(&document::Range::from(0, 19, 0, 19))).unwrap();
    assert_eq!(doc.text(), r#"let 兄弟 = vec!["Ken", "Chad"];"#);

    //let tree = parser.parse(doc.text(), None).unwrap();
    //println!("{}", language::pretty_print(&tree.root_node(), &doc));

}






