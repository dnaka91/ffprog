use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::Result;
use bincode::{config, BorrowDecode, Decode, Encode};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use time::Duration;

use crate::{ffmpeg::Progress, ffprobe::Format};

pub struct Stats {
    pub import: Format,
    pub history: Vec<(Duration, Progress)>,
}

#[derive(Encode, Decode)]
enum Version {
    V1 {
        import: FormatV1,
        history: Vec<(BincodeDuration, ProgressV1)>,
    },
}

impl From<&Stats> for Version {
    fn from(s: &Stats) -> Self {
        Self::V1 {
            import: s.import.clone().into(),
            history: s
                .history
                .iter()
                .cloned()
                .map(|(d, p)| (d.into(), p.into()))
                .collect(),
        }
    }
}

impl From<Version> for Stats {
    fn from(v: Version) -> Self {
        match v {
            Version::V1 { import, history } => Stats {
                import: import.into(),
                history: history
                    .into_iter()
                    .map(|(d, p)| (d.into(), p.into()))
                    .collect(),
            },
        }
    }
}

#[derive(Encode, Decode)]
struct FormatV1 {
    pub filename: String,
    pub nb_streams: u32,
    pub nb_programs: u32,
    pub format_name: String,
    pub format_long_name: Option<String>,
    pub start_time: BincodeDuration,
    pub duration: BincodeDuration,
    pub size: u64,
    pub bit_rate: u64,
    pub probe_score: u8,
    pub tags: BTreeMap<String, String>,
}

impl From<Format> for FormatV1 {
    fn from(f: Format) -> Self {
        Self {
            filename: f.filename,
            nb_streams: f.nb_streams,
            nb_programs: f.nb_programs,
            format_name: f.format_name,
            format_long_name: f.format_long_name,
            start_time: f.start_time.into(),
            duration: f.duration.into(),
            size: f.size,
            bit_rate: f.bit_rate,
            probe_score: f.probe_score,
            tags: f.tags,
        }
    }
}

impl From<FormatV1> for Format {
    fn from(f: FormatV1) -> Self {
        Self {
            filename: f.filename,
            nb_streams: f.nb_streams,
            nb_programs: f.nb_programs,
            format_name: f.format_name,
            format_long_name: f.format_long_name,
            start_time: f.start_time.into(),
            duration: f.duration.into(),
            size: f.size,
            bit_rate: f.bit_rate,
            probe_score: f.probe_score,
            tags: f.tags,
        }
    }
}

#[derive(Encode, Decode)]
struct ProgressV1 {
    pub frame: u64,
    pub fps: f64,
    pub bitrate: u64,
    pub total_size: u64,
    pub out_time_us: u64,
    pub out_time_ms: u64,
    pub out_time: BincodeDuration,
    pub dup_frames: u64,
    pub drop_frames: u64,
    pub speed: f64,
}

impl From<Progress> for ProgressV1 {
    fn from(p: Progress) -> Self {
        Self {
            frame: p.frame,
            fps: p.fps,
            bitrate: p.bitrate,
            total_size: p.total_size,
            out_time_us: p.out_time_us,
            out_time_ms: p.out_time_ms,
            out_time: p.out_time.into(),
            dup_frames: p.dup_frames,
            drop_frames: p.drop_frames,
            speed: p.speed,
        }
    }
}

impl From<ProgressV1> for Progress {
    fn from(p: ProgressV1) -> Self {
        Self {
            frame: p.frame,
            fps: p.fps,
            bitrate: p.bitrate,
            total_size: p.total_size,
            out_time_us: p.out_time_us,
            out_time_ms: p.out_time_ms,
            out_time: p.out_time.into(),
            dup_frames: p.dup_frames,
            drop_frames: p.drop_frames,
            speed: p.speed,
        }
    }
}

pub fn save(stats: &Stats, input: &Path) -> Result<()> {
    let input = {
        let mut os_str = input.as_os_str().to_os_string();
        os_str.push(".stats");
        PathBuf::from(os_str)
    };

    let mut dst = GzEncoder::new(BufWriter::new(File::create(input)?), Compression::best());
    let version = Version::from(stats);

    bincode::encode_into_std_write(version, &mut dst, config::standard())?;

    dst.finish()?.into_inner()?.flush()?;

    Ok(())
}

pub fn load(input: &Path) -> Result<Stats> {
    let input = {
        let mut os_str = input.as_os_str().to_os_string();
        os_str.push(".stats");
        PathBuf::from(os_str)
    };

    let mut src = GzDecoder::new(BufReader::new(File::open(input)?));
    let version = bincode::decode_from_std_read::<Version, _, _>(&mut src, config::standard())?;

    Ok(version.into())
}

struct BincodeDuration(Duration);

impl Encode for BincodeDuration {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        self.0.whole_seconds().encode(encoder)?;
        self.0.subsec_nanoseconds().encode(encoder)?;
        Ok(())
    }
}

impl Decode for BincodeDuration {
    fn decode<D: bincode::de::Decoder>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self(
            Duration::seconds(i64::decode(decoder)?) + Duration::nanoseconds(i64::decode(decoder)?),
        ))
    }
}

impl<'de> BorrowDecode<'de> for BincodeDuration {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self(
            Duration::seconds(i64::borrow_decode(decoder)?)
                + Duration::nanoseconds(i64::borrow_decode(decoder)?),
        ))
    }
}

impl From<Duration> for BincodeDuration {
    fn from(d: Duration) -> Self {
        Self(d)
    }
}

impl From<BincodeDuration> for Duration {
    fn from(d: BincodeDuration) -> Self {
        d.0
    }
}
