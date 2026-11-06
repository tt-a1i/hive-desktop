use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

// Real read-only Hive authentication + workspace probe. Deliberately loopback-only.
pub fn probe_local(endpoint: &str) -> String {
    let run = || -> Result<String, Box<dyn std::error::Error>> {
        let authority = endpoint
            .strip_prefix("http://")
            .ok_or("仅支持 http://loopback-IP:PORT")?
            .trim_end_matches('/');
        let address: SocketAddr = authority.parse()?;
        if !address.ip().is_loopback() {
            return Err("仅支持 loopback IP".into());
        }
        let session = get(address, authority, "/api/ui/session", None)?;
        if !session.lines().next().unwrap_or("").contains(" 200 ") {
            return Err("UI session 未返回 HTTP 200".into());
        }
        let cookie = session
            .lines()
            .filter_map(|line| line.split_once(':'))
            .filter(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
            .map(|(_, value)| value.trim().split(';').next().unwrap_or(""))
            .find(|cookie| cookie.starts_with("hive_ui_token="))
            .ok_or("UI session 未返回 hive_ui_token cookie")?;
        let response = get(address, authority, "/api/workspaces", Some(cookie))?;
        let status = response.lines().next().unwrap_or("空 HTTP 响应");
        let body = response
            .split_once("\r\n\r\n")
            .map(|(_, body)| body)
            .unwrap_or("");
        Ok(format!(
            "session 200 → {status} · {}",
            body.chars().take(120).collect::<String>()
        ))
    };
    run().unwrap_or_else(|error| format!("GET 失败：{error}"))
}
fn get(
    address: SocketAddr,
    authority: &str,
    path: &str,
    cookie: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(3))?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let cookie_header = cookie
        .map(|cookie| format!("Cookie: {cookie}\r\n"))
        .unwrap_or_default();
    write!(
        stream,
        "GET {path} HTTP/1.0\r\nHost: {authority}\r\n{cookie_header}Connection: close\r\n\r\n"
    )?;
    let mut bytes = Vec::new();
    stream.take(65536).read_to_end(&mut bytes)?;
    Ok(String::from_utf8(bytes)?)
}
