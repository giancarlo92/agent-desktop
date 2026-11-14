use super::*;
use clap::Parser;

fn command(args: &[&str]) -> Commands {
    let mut argv = vec!["agent-desktop"];
    argv.extend_from_slice(args);
    crate::cli::Cli::try_parse_from(argv)
        .expect("parses")
        .command
        .expect("has a command")
}

fn refused(args: &[&str]) -> bool {
    refusal(&command(args)).is_some()
}

#[test]
fn physical_and_foreground_commands_are_refused() {
    assert!(refused(&["press", "return"]));
    assert!(refused(&["focus-window", "--app", "notepad.exe"]));
    assert!(refused(&["mouse-click", "--xy", "10,10"]));
    assert!(refused(&["launch", "notepad.exe", "--activate"]));
}

#[test]
fn background_commands_are_not_refused() {
    assert!(!refused(&["click", "@s1abc:e1"]));
    assert!(!refused(&["set-value", "@s1abc:e1", "text"]));
    assert_eq!(
        refused(&["type", "@s1abc:e1", "text"]),
        cfg!(target_os = "windows")
    );
    assert!(!refused(&["launch", "notepad.exe"]));
    assert!(!refused(&["list-windows"]));
}

#[test]
fn every_refused_name_is_a_command() {
    use clap::CommandFactory;
    let root = crate::cli::Cli::command();
    for (name, _) in REFUSED {
        assert!(
            root.get_subcommands().any(|sub| sub.get_name() == *name),
            "{name} is not a command"
        );
    }
}

#[test]
fn the_refusal_says_why_and_what_to_do() {
    let AppError::Adapter(error) = refusal(&command(&["press", "return"])).unwrap() else {
        panic!("an adapter error");
    };
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert!(error.message.contains("background"));
    assert!(error.suggestion.is_some());
}
