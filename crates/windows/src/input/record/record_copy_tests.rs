use super::*;

#[test]
fn only_ctrl_c_and_ctrl_x_are_copies() {
    assert!(is_copy("ctrl+c"));
    assert!(is_copy("ctrl+shift+c"));
    assert!(is_copy("ctrl+x"));
    assert!(!is_copy("c"));
    assert!(!is_copy("ctrl+v"));
    assert!(!is_copy("alt+c"));
}

#[test]
fn a_table_is_measured_in_lines_and_columns() {
    let table = "1\tAna\tana@x.com\r\n2\tLuis\tluis@x.com\r\n3\tEva\teva@x.com\r\n";
    assert_eq!(shape(table)["lines"], 3);
    assert_eq!(shape(table)["columns"], 3);
    assert_eq!(shape("1762621")["lines"], 1);
    assert_eq!(shape("1762621")["columns"], 1);
    assert_eq!(shape("")["lines"], 0);
}
