use super::*;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn leader_exit_is_detected_while_descendants_hold_stdout() {
    let directory = std::env::temp_dir().join(format!(
        "hive-sidecar-watch-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let script = directory.join("fixture.cjs");
    fs::write(&script, r#"
const {spawn}=require('node:child_process');
const fs=require('node:fs');
const path=require('node:path');
const role=process.argv[2], dir=process.argv[3];
if(role==='grandchild') { fs.writeFileSync(path.join(dir,'grandchild.pid'),String(process.pid)); setInterval(()=>{},1000); }
else if(role==='child') {
 fs.writeFileSync(path.join(dir,'child.pid'),String(process.pid));
 spawn(process.execPath,[__filename,'grandchild',dir],{stdio:['ignore',process.stdout,process.stderr]}); setInterval(()=>{},1000);
} else {
 require('node:readline').createInterface({input:process.stdin}).on('line',line=>{
  const q=JSON.parse(line);
  if(q.method==='initialize') {
   spawn(process.execPath,[__filename,'child',dir],{stdio:['ignore',process.stdout,process.stderr]});
   process.stdout.write(JSON.stringify({id:q.id,result:{ready:true}})+'\n');
  } else if(q.method==='crash') process.exit(17);
 });
}
"#).unwrap();
    let mut command = Command::new("node");
    command
        .arg(&script)
        .arg("leader")
        .arg(&directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let runtime = Runtime::launch_command(command).unwrap();
    let result = smol::block_on(async {
        assert_eq!(
            runtime.request("initialize", json!({})).await.unwrap()["ready"],
            true
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !directory.join("grandchild.pid").exists() {
            assert!(std::time::Instant::now() < deadline);
            smol::Timer::after(Duration::from_millis(10)).await;
        }
        // Node exits without answering; its child/grandchild still own stdout.
        // Only the independent leader watcher can break this pending request.
        let request = runtime.request("crash", json!({}));
        let reply = smol::future::race(async { Some(request.await) }, async {
            smol::Timer::after(Duration::from_secs(5)).await;
            None
        })
        .await
        .expect("Leader watcher did not settle the pending request while stdout remained open");
        assert!(reply.is_err());
        runtime.shutdown_checked(Duration::ZERO).await
    })
    .unwrap();
    assert!(result.forced);
    let child = fs::read_to_string(directory.join("child.pid")).unwrap();
    let grandchild = fs::read_to_string(directory.join("grandchild.pid")).unwrap();
    let verified = Command::new("node").arg("-e").arg("for(const pid of process.argv.slice(1)){try{process.kill(Number(pid),0);process.exit(1)}catch(e){if(e.code!=='ESRCH')throw e}}")
        .arg(child).arg(grandchild).status().unwrap();
    assert!(
        verified.success(),
        "A contained descendant survived the transport exit barrier"
    );
    drop(runtime);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn concurrent_shutdown_callers_wait_for_same_real_child_exit() {
    let directory = std::env::temp_dir().join(format!(
        "hive-sidecar-concurrent-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let script = directory.join("gate.cjs");
    fs::write(&script, r#"
const fs=require('node:fs'),path=require('node:path'),dir=process.argv[2];
require('node:readline').createInterface({input:process.stdin}).on('line',line=>{
 const q=JSON.parse(line);
 if(q.method==='initialize')process.stdout.write(JSON.stringify({id:q.id,result:{ready:true}})+'\n');
 if(q.method==='shutdown')fs.appendFileSync(path.join(dir,'shutdown-seen'),'request\n');
});
setInterval(()=>{if(fs.existsSync(path.join(dir,'release-exit')))process.exit(0)},10);
"#).unwrap();
    let mut command = Command::new("node");
    command
        .arg(&script)
        .arg(&directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let runtime = Runtime::launch_command(command).unwrap();
    smol::block_on(async {
        assert_eq!(
            runtime.request("initialize", json!({})).await.unwrap()["ready"],
            true
        );
        let first = runtime.shutdown_checked(Duration::from_secs(10));
        let second = runtime.shutdown_checked(Duration::from_secs(10));
        let mut first = std::pin::pin!(first);
        let mut second = std::pin::pin!(second);
        assert!(smol::future::poll_once(first.as_mut()).await.is_none());
        assert!(smol::future::poll_once(second.as_mut()).await.is_none());
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while fs::read_to_string(directory.join("shutdown-seen"))
            .unwrap_or_default()
            .lines()
            .count()
            < 2
        {
            assert!(
                std::time::Instant::now() < deadline,
                "Both cleanup callers did not reach the real child"
            );
            smol::Timer::after(Duration::from_millis(10)).await;
        }
        // The real child acknowledged both intents and is held by an external
        // gate. Neither caller may report completion merely because another owns cleanup.
        assert!(smol::future::poll_once(first.as_mut()).await.is_none());
        assert!(smol::future::poll_once(second.as_mut()).await.is_none());
        fs::write(directory.join("release-exit"), "release").unwrap();
        let one = first.await.unwrap();
        let two = second.await.unwrap();
        assert_eq!(one, two);
        assert!(
            !one.forced,
            "Neither caller may force termination before the gate is released"
        );
    });
    drop(runtime);
    fs::remove_dir_all(directory).unwrap();
}
