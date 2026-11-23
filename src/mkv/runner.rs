//! Process execution with captured output and a hard timeout.

use std::ffi::OsString;
use std::io::Read;
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

use super::tools::ToolCommand;

#[derive(Clone, Debug, Default)]
pub struct ToolOutput {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl ToolOutput {
    pub fn success(&self) -> bool {
        self.status == Some(0) && !self.timed_out
    }

    pub fn combined(&self) -> String {
        let mut text = String::new();
        if !self.stdout.trim().is_empty() {
            text.push_str(self.stdout.trim());
        }
        if !self.stderr.trim().is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(self.stderr.trim());
        }
        text
    }
}

pub fn run(tool: &ToolCommand, args: &[OsString], timeout: Duration) -> Result<ToolOutput, String> {
    let mut child = tool
        .command()
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not start {}: {error}", tool.display()))?;

    let stdout_handle = child.stdout.take().map(|mut stream| {
        thread::spawn(move || {
            let mut buffer = String::new();
            let _ = stream.read_to_string(&mut buffer);
            buffer
        })
    });
    let stderr_handle = child.stderr.take().map(|mut stream| {
        thread::spawn(move || {
            let mut buffer = String::new();
            let _ = stream.read_to_string(&mut buffer);
            buffer
        })
    });

    let started = Instant::now();
    let mut timed_out = false;
    let mut exit_code: Option<i32> = None;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_code = status.code();
                break;
            }
            Ok(None) => {
                if started.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break;
                }
                thread::sleep(Duration::from_millis(40));
            }
            Err(error) => {
                return Err(format!("Could not wait for {}: {error}", tool.display()));
            }
        }
    }

    let stdout = stdout_handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();
    let stderr = stderr_handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();

    Ok(ToolOutput {
        status: exit_code,
        stdout,
        stderr,
        timed_out,
    })
}
