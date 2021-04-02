
use ls_core;
use ls_core::ts_interface;

#[test]
fn test_dbl() {
    assert_eq!(ls_core::dbl(10.0), 20.0);
}

#[allow(dead_code)]
#[test]
fn test_tree_sitter() {
    let mut parser = ts_interface::get_parser("rs").unwrap();
    let code = r#"use ls_core;
    use ls_core::ts_interface;

    #[test]
    fn test_dbl() {
        assert_eq!(ls_core::dbl(10.0), 20.0);
    }

    fn double(x: i32) -> i32 {
        x + 2
    }
    "#;

    let parsed = parser.parse(code, None).unwrap();
    println!("{}", ts_interface::pretty_print(&parsed.root_node()));
}