use std::{
    io::{BufRead, BufReader},
    process::{Child, ChildStdout, Command, Stdio},
    time::Duration,
};

use anyhow::{ensure, Context, Result};

pub struct ProgressIter {
    child: Option<Child>,
    reader: BufReader<ChildStdout>,
}

impl Iterator for ProgressIter {
    type Item = Result<Progress>;

    fn next(&mut self) -> Option<Self::Item> {
        self.child.as_ref()?;

        let mut progress = Progress::default();
        let mut buf = String::new();

        loop {
            buf.clear();

            match self.reader.read_line(&mut buf) {
                Ok(0) => {
                    return match finish_process(self.child.take()) {
                        Ok(()) => None,
                        Err(e) => Some(Err(e)),
                    };
                }
                Ok(_) => match buf.trim().split_once('=') {
                    Some((key, value)) => match parse_kv(&mut progress, key, value) {
                        Ok(true) => return Some(Ok(progress)),
                        Ok(false) => continue,
                        Err(e) => return Some(Err(e)),
                    },
                    None => continue,
                },
                Err(e) => return Some(Err(e.into())),
            }
        }
    }
}

impl Drop for ProgressIter {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            child.kill().ok();
        }
    }
}

fn parse_kv(progress: &mut Progress, key: &str, value: &str) -> Result<bool> {
    let value = value.trim();
    match key {
        "frame" => progress.frame = value.parse()?,
        "fps" => progress.fps = value.parse()?,
        "bitrate" => {
            progress.bitrate = (value
                .strip_suffix("kbits/s")
                .unwrap_or(value)
                .parse::<f64>()?
                * 1000.0) as u64
        }
        "total_size" => progress.total_size = value.parse()?,
        "out_time_us" => progress.out_time_us = value.parse()?,
        "out_time_ms" => progress.out_time_ms = value.parse()?,
        "out_time" => progress.out_time = parse_time(value)?,
        "dup_frames" => progress.dup_frames = value.parse()?,
        "drop_frames" => progress.drop_frames = value.parse()?,
        "speed" => progress.speed = value.strip_suffix('x').unwrap_or(value).parse()?,
        "progress" => return Ok(true),
        _ => return Ok(false),
    }

    Ok(false)
}

fn parse_time(value: &str) -> Result<Duration> {
    let (hours, value) = value.split_once(':').context("hours missing")?;
    let (minutes, value) = value.split_once(':').context("minutes missing")?;
    let (seconds, micros) = value.split_once('.').context("seconds missing")?;

    let total_seconds =
        hours.parse::<u64>()? * 3600 + minutes.parse::<u64>()? * 60 + seconds.parse::<u64>()?;

    Ok(Duration::from_secs(total_seconds) + Duration::from_micros(micros.parse()?))
}

fn finish_process(child: Option<Child>) -> Result<()> {
    let child = match child {
        Some(c) => c,
        None => return Ok(()),
    };

    let output = child.wait_with_output()?;

    ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

#[derive(Clone, Default)]
pub struct Progress {
    pub frame: u64,
    pub fps: f64,
    pub bitrate: u64,
    pub total_size: u64,
    pub out_time_us: u64,
    pub out_time_ms: u64,
    pub out_time: Duration,
    pub dup_frames: u64,
    pub drop_frames: u64,
    pub speed: f64,
}

pub fn spawn(args: &[String], overwrite: bool) -> Result<ProgressIter> {
    let mut child = Command::new("ffmpeg")
        .args([
            "-progress",
            "pipe:1",
            "-nostats",
            "-nostdin",
            "-hide_banner",
            "-stats_period",
            "0.5",
            "-loglevel",
            "warning",
        ])
        .arg(if overwrite { "-y" } else { "-n" })
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child
        .stdout
        .take()
        .context("failed taking stdout from ffmpeg")?;

    Ok(ProgressIter {
        child: Some(child),
        reader: BufReader::new(stdout),
    })
}
