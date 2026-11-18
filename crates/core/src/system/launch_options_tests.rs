// Modified in the desktop-recorder-mcp fork of agent-desktop (see NOTICE).
use super::*;

#[test]
fn default_preserves_attach_if_running_semantics() {
    let options = LaunchOptions::default();

    assert!(
        options.attach_if_running,
        "default LaunchOptions must attach to an already-running instance, matching \
         launch_app's historical behavior; a derived Default would silently flip this to \
         false and turn every unmodified caller into a --no-attach launch"
    );
    assert!(options.args.is_empty());
    assert!(options.env.is_empty());
    assert!(options.cwd.is_none());
}

#[test]
fn explicit_no_attach_overrides_the_default() {
    let options = LaunchOptions {
        attach_if_running: false,
        ..Default::default()
    };

    assert!(!options.attach_if_running);
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|arg| (*arg).to_owned()).collect()
}

#[test]
fn a_profile_directory_of_its_own_makes_a_separate_instance() {
    assert!(has_own_profile(&args(&[
        "--user-data-dir=C:/bot",
        "about:blank"
    ])));
    assert!(!has_own_profile(&args(&["--new-window", "about:blank"])));
    assert!(has_own_profile(&args(&[
        "-data",
        "C:/bot/workspace",
        "-nosplash"
    ])));
    assert!(!has_own_profile(&args(&["-data", "-nosplash"])));
    assert!(!has_own_profile(&args(&["-data"])));
}

#[test]
fn the_pages_are_taken_out_of_a_browsers_arguments() {
    let mut list = args(&[
        "--no-first-run",
        "--new-window",
        "https://a.example/",
        "https://b.example/",
    ]);
    assert_eq!(
        take_pages(&mut list),
        ["https://a.example/", "https://b.example/"]
    );
    assert_eq!(list, ["--no-first-run"]);

    let mut none = args(&["--no-first-run"]);
    assert_eq!(take_pages(&mut none), ["about:blank"]);
}

#[test]
fn a_covered_browser_keeps_working_without_losing_the_callers_switches() {
    let mut plain = args(&["--no-first-run"]);
    keep_working_while_covered(&mut plain);
    assert!(plain.contains(&"--disable-features=CalculateNativeWinOcclusion".to_owned()));
    assert!(plain.contains(&"--disable-backgrounding-occluded-windows".to_owned()));

    let mut merged = args(&[
        "--disable-features=Translate",
        "--disable-renderer-backgrounding",
    ]);
    keep_working_while_covered(&mut merged);
    assert_eq!(
        merged[0],
        "--disable-features=Translate,CalculateNativeWinOcclusion"
    );
    assert_eq!(
        merged
            .iter()
            .filter(|arg| *arg == "--disable-renderer-backgrounding")
            .count(),
        1
    );
}
