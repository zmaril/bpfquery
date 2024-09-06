
use openssh::{KnownHosts, SessionBuilder};

pub async fn get_session() -> openssh::Session {
    let mut s = SessionBuilder::default();
    let mut h = std::env::var("BPFTRACE_MACHINE").unwrap();
    let user = "root".to_string();
    h = format!("{}@{}", user, h);
    s.keyfile("bpftrace_machine");
    s.known_hosts_check(KnownHosts::Accept);
    s.connect(h).await.unwrap()
}
