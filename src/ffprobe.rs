use std::{collections::BTreeMap, path::Path, process::Command};

use anyhow::{ensure, Result};
use serde::Deserialize;
use time::Duration;

#[derive(Deserialize)]
struct Report {
    format: Format,
}

#[derive(Clone, Deserialize)]
pub struct Format {
    pub filename: String,
    pub nb_streams: u32,
    pub nb_programs: u32,
    pub format_name: String,
    pub format_long_name: Option<String>,
    #[serde(deserialize_with = "de::duration")]
    pub start_time: Duration,
    #[serde(deserialize_with = "de::duration")]
    pub duration: Duration,
    #[serde(deserialize_with = "de::from_str")]
    pub size: u64,
    #[serde(deserialize_with = "de::from_str")]
    pub bit_rate: u64,
    pub probe_score: u8,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
}

pub fn run(input: &Path) -> Result<Format> {
    let output = Command::new("ffprobe")
        .args([
            "-hide_banner",
            "-print_format",
            "json=compact=1",
            "-show_streams",
            "-show_format",
        ])
        .arg("-i")
        .arg(input)
        .output()?;

    ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice::<Report>(&output.stdout)
        .map(|r| r.format)
        .map_err(Into::into)
}

mod de {
    use std::{
        fmt::{self, Display},
        marker::PhantomData,
        str::FromStr,
    };

    use serde::de::{self, Deserializer, Visitor};
    use time::Duration;

    pub fn from_str<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: FromStr,
        T::Err: Display,
    {
        deserializer.deserialize_str(FromStrVisitor::default())
    }

    struct FromStrVisitor<T> {
        ty: PhantomData<T>,
    }

    impl<T> Default for FromStrVisitor<T> {
        fn default() -> Self {
            Self {
                ty: PhantomData::default(),
            }
        }
    }

    impl<'de, T> Visitor<'de> for FromStrVisitor<T>
    where
        T: FromStr,
        T::Err: Display,
    {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("any value that can be parse from its string representation")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            T::from_str(v).map_err(E::custom)
        }
    }

    pub fn duration<'de, D>(deserialize: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize.deserialize_str(DurationVisitor)
    }

    struct DurationVisitor;

    impl<'de> Visitor<'de> for DurationVisitor {
        type Value = Duration;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("duration encoded as total seconds plus fraction")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            FromStrVisitor::<f64>::default()
                .visit_str(v)
                .map(Duration::seconds_f64)
        }
    }
}
