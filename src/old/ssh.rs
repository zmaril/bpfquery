use ssh2::Session;
use std::net::TcpStream;
use std::io::prelude::*;
use ssh2_config::{ParseRule, SshConfig};
use std::fs::File;
use std::io::BufReader;

pub fn get_session(hostname: String) -> Session {
    let mut reader = BufReader::new(File::open("/Users/zackmaril/.ssh/config").expect("Could not open configuration file"));
    let config = SshConfig::default().parse(&mut reader, ParseRule::STRICT).expect("Failed to parse configuration");

    // Query attributes for a certain host
    let params = config.query(hostname.clone());

    let h = params.host_name.unwrap();
    let port = params.port.unwrap_or(22);


    let tcp = TcpStream::connect(format!("{}:{}", h, port)).unwrap();

    let mut sess = Session::new().unwrap();
    sess.set_tcp_stream(tcp);
    sess.handshake().unwrap();

    let user = params.user.unwrap_or("root".to_string());
    sess.userauth_agent(&user).unwrap();
    sess
}
