//! The session's own pieces: framing, quoting and the sign-in's steps.
//! (A file of their own: the code scan leaves test files' fixed passwords
//! alone.)

use super::*;

#[test]
fn literals_are_found_at_the_end_of_a_line() {
    assert_eq!(literal_size(b"* 1 FETCH (BODY[] {12}\r\n"), Some(12));
    assert_eq!(literal_size(b"* 1 FETCH (BODY[] {12+}\n"), Some(12));
    assert_eq!(literal_size(b"* LIST () \"/\" \"a{1}\"\r\n"), None);
    assert_eq!(literal_size(b"* 1 FETCH (BODY[] {}\r\n"), None);
    assert_eq!(
        literal_size(b"* 1 FETCH (BODY[] {123456789012345678901}\r\n"),
        None
    );
}

#[test]
fn only_plain_ascii_is_quoted() {
    assert_eq!(quoted("Sent Items").as_deref(), Some("\"Sent Items\""));
    assert_eq!(quoted("a\"b\\c").as_deref(), Some("\"a\\\"b\\\\c\""));
    assert_eq!(quoted("line\r\nA1 DELETE INBOX"), None);
    assert_eq!(quoted("Entw\u{fc}rfe"), None);
}

#[test]
fn a_busy_server_is_told_from_a_refusal() {
    let done = |code: &str| Done {
        status: Status::No,
        code: Some(Code {
            name: code.to_owned(),
            number: None,
            sets: Vec::new(),
        }),
    };
    assert_eq!(sign_in_refusal(&done("UNAVAILABLE")), Refusal::ImapBusy);
    assert_eq!(sign_in_refusal(&done("INUSE")), Refusal::ImapBusy);
    assert_eq!(
        sign_in_refusal(&done("AUTHENTICATIONFAILED")),
        Refusal::ImapLoginRefused
    );
    let bare = Done {
        status: Status::Bad,
        code: None,
    };
    assert_eq!(sign_in_refusal(&bare), Refusal::ImapLoginRefused);
}

#[test]
fn plain_carries_the_username_and_password_once() {
    let password = SecretString::from("app-password");
    let response = plain("ana@example.com", &password);
    assert_eq!(
        STANDARD.decode(response.as_str()).unwrap(),
        b"\0ana@example.com\0app-password"
    );
}

#[test]
fn login_quotes_what_it_can_and_sends_the_rest_as_literals() {
    let text = |steps: Vec<Zeroizing<Vec<u8>>>| -> Vec<String> {
        steps
            .iter()
            .map(|s| String::from_utf8(s.to_vec()).unwrap())
            .collect()
    };
    assert_eq!(
        text(login("w1", "ana", &SecretString::from("p\"w"))),
        ["w1 LOGIN \"ana\" \"p\\\"w\"\r\n"]
    );
    assert_eq!(
        text(login("w1", "ana", &SecretString::from("p\u{e4}ss"))),
        ["w1 LOGIN {3}\r\n", "ana {5}\r\n", "p\u{e4}ss\r\n"]
    );
}

#[test]
fn only_known_response_codes_are_logged() {
    let code = |name: &str| Code {
        name: name.to_owned(),
        number: None,
        sets: Vec::new(),
    };
    assert_eq!(logged_code(None), "");
    assert_eq!(
        logged_code(Some(&code("AUTHENTICATIONFAILED"))),
        "AUTHENTICATIONFAILED"
    );
    assert_eq!(logged_code(Some(&code("UNAVAILABLE"))), "UNAVAILABLE");
    // Whatever else a server puts there is its own word: never logged.
    assert_eq!(logged_code(Some(&code("ANA-AT-BAKERY"))), "other");
    assert!(LOGGED_CODES.windows(2).all(|pair| pair[0] < pair[1]));
}
