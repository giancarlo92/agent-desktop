use super::*;

fn targets() -> Value {
    json!([
        { "type": "page", "title": "Sign in", "url": "http://127.0.0.1:8765/login.html",
          "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/page/A" },
        { "type": "page", "title": "Inbox", "url": "https://mail.example/#inbox",
          "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/page/B" },
        { "type": "service_worker", "title": "Sign in", "url": "http://127.0.0.1:8765/login.html",
          "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/page/C" },
        { "type": "page", "title": "Inspected", "url": "https://busy.example/" }
    ])
}

fn pages(url: Option<&str>, title: Option<&str>) -> Vec<String> {
    matching_pages(&targets(), &PageQuery { url, title })
}

#[test]
fn a_page_is_found_by_its_address_without_the_fragment() {
    assert_eq!(
        pages(Some("http://127.0.0.1:8765/login.html#top"), None),
        ["ws://127.0.0.1:9222/devtools/page/A"]
    );
    assert_eq!(
        pages(Some("https://mail.example/#sent"), Some("Something else")),
        ["ws://127.0.0.1:9222/devtools/page/B"]
    );
}

#[test]
fn the_title_decides_only_when_no_address_matches() {
    assert_eq!(
        pages(Some("http://elsewhere.example/"), Some("Inbox")),
        ["ws://127.0.0.1:9222/devtools/page/B"]
    );
    assert_eq!(
        pages(None, Some("Sign in")),
        ["ws://127.0.0.1:9222/devtools/page/A"]
    );
}

#[test]
fn nothing_matches_without_an_address_or_a_title_or_a_free_endpoint() {
    assert!(pages(None, None).is_empty());
    assert!(pages(Some(""), Some("")).is_empty());
    assert!(pages(Some("https://busy.example/"), None).is_empty());
    assert!(
        matching_pages(
            &json!({}),
            &PageQuery {
                url: None,
                title: Some("Inbox")
            }
        )
        .is_empty()
    );
}

#[test]
fn the_page_answer_says_whether_it_ran_fell_back_or_had_no_such_option() {
    assert_eq!(outcome(&json!(true)), Outcome::Done { verified: true });
    assert_eq!(outcome(&json!(false)), Outcome::Done { verified: false });
    assert_eq!(outcome(&json!("other")), Outcome::NotFound);
    assert_eq!(
        outcome(&json!({ "options": ["Choose a country…", "Spain", "Peru"] })),
        Outcome::NoSuchOption(vec![
            "Choose a country…".into(),
            "Spain".into(),
            "Peru".into()
        ])
    );
}
