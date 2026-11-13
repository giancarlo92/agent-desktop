use super::*;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ad-record-bg-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn session(id: &str) -> Session {
    Session {
        id: id.into(),
        pid: 1,
        out: PathBuf::from(format!("{id}.jsonl")),
        video: None,
        stop_file: PathBuf::from(format!("{id}.stop")),
    }
}

#[test]
fn stop_takes_the_only_running_recording_or_the_one_named() {
    let one = [session("rec-1")];
    assert_eq!(choose(&one, None).unwrap().id, "rec-1");
    let two = [session("rec-1"), session("rec-2")];
    assert_eq!(choose(&two, Some("rec-2")).unwrap().id, "rec-2");
    assert!(choose(&two, None).is_err(), "two running need an id");
    assert!(choose(&[], None).is_err());
    assert!(choose(&two, Some("rec-9")).is_err());
}

#[test]
fn sessions_are_read_back_from_their_state_files() {
    let dir = Scratch::new("sessions");
    write_state(dir.path(), &session("rec-2")).unwrap();
    write_state(dir.path(), &session("rec-1")).unwrap();
    std::fs::write(dir.path().join("rec-1.jsonl"), b"{}").unwrap();
    let found = sessions(dir.path());
    let ids: Vec<&str> = found.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["rec-1", "rec-2"]);
}

#[test]
fn a_new_id_never_reuses_a_running_one() {
    let dir = Scratch::new("id");
    let first = new_id(dir.path());
    write_state(dir.path(), &session(&first)).unwrap();
    assert_ne!(new_id(dir.path()), first);
}

#[test]
fn the_trailer_is_read_from_the_last_matching_line() {
    let dir = Scratch::new("trailer");
    let path = dir.path().join("r.jsonl");
    std::fs::write(
        &path,
        "{\"kind\":\"recording_ready\",\"t_ms\":1}\n{\"kind\":\"click\",\"t_ms\":5}\n{\"kind\":\"recording_stopped\",\"t_ms\":9,\"events\":1}\n",
    )
    .unwrap();
    assert!(has_line(&path, RECORDING_READY_KIND));
    let trailer = last_line_of_kind(&path, "recording_stopped").unwrap();
    assert_eq!(trailer["events"], 1);
}
