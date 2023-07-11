pub mod mb;

use std::{
    ffi::{c_int, c_long, CString},
    io::SeekFrom,
    path::PathBuf,
    time::Instant,
};

use aho_corasick::AhoCorasick;
use anyhow::{anyhow, bail, Context};
use cdparanoia::{CdromDrive, CdromParanoia, ParanoiaMode, CD_FRAMEWORDS};
use clap::Parser;
use dialoguer::{theme::ColorfulTheme, Confirm, Select};
use discid::DiscId;
use flac_bound::FlacEncoder;

const CD_SAMPLE_RATE: u32 = 44100;

#[derive(clap::Parser)]
pub struct Cli {
    /// Path to CD-ROM device. (default: /dev/cdrom)
    #[arg(short, long)]
    disc_device: Option<String>,

    /// Base path for output files.
    output_path: PathBuf,

    #[arg(short, long)]
    ntfs_filenames: bool,
}

enum PathSanitizer {
    Default,
    Ntfs(AhoCorasick),
}

impl PathSanitizer {
    pub fn default() -> Self {
        Self::Default
    }

    pub fn ntfs() -> Self {
        Self::Ntfs(AhoCorasick::new(["/", ":", "?", "\"", "|", "*"]).unwrap())
    }

    pub fn map<'a>(&self, filename: &'a str) -> String {
        match self {
            Self::Default => filename.replace("/", "\u{2215}"),
            Self::Ntfs(matcher) => matcher.replace_all(
                filename,
                &[
                    "\u{2215}", "\u{02d0}", "\u{0294}", "\u{00a8}", "\u{01c0}", "\u{04ff}",
                ],
            ),
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let path_sanitizer = if args.ntfs_filenames {
        PathSanitizer::default()
    } else {
        PathSanitizer::ntfs()
    };

    let disc_device = args.disc_device.as_deref().unwrap_or("/dev/cdrom");

    let disc_info = DiscId::read(Some(disc_device))?;
    let disc_id = disc_info.id();
    let toc = disc_info.toc_string();

    println!("Disc ID: {:?}", disc_id);
    println!("TOC: {:?}", toc);

    let mb_client = mb::Client::new();
    let mb_info = mb::DiscId::lookup(&mb_client, &disc_id)?;
    if mb_info.releases.is_empty() {
        bail!("No release found with this MBID. Please submit it to the database.");
    }

    let release_summaries: Vec<String> = mb_info
        .releases
        .iter()
        .map(|release| {
            let mbid = &release.id;
            let catalog_number = release
                .label_info
                .get(0)
                .map(|label| label.catalog_number.as_str())
                .unwrap_or("");
            let barcode = release.barcode.as_deref().unwrap_or("");
            let artist = release.artist_string();
            let title = &release.title;
            format!(
                "MBID: {}\
                \n  - Artist: {}\
                \n  - Title: {}\
                \n  - Catalog Number: {}\
                \n  - Barcode: {}",
                mbid, artist, title, catalog_number, barcode
            )
        })
        .collect();

    let console_theme = ColorfulTheme::default();

    let selected_index = Select::with_theme(&console_theme)
        .with_prompt("Confirm release:")
        .items(&release_summaries)
        .interact()?;
    let selected_release = &mb_info.releases[selected_index];
    let multi_disc = selected_release.media.len() > 1;
    let mb_disc_info = selected_release
        .media
        .iter()
        .find(|medium| medium.discs.iter().any(|disc| disc.id == disc_id))
        .context("Cannot find medium that matches the disc ID")?;

    let dir_name = path_sanitizer.map(&format!(
        "{} - {}",
        selected_release.artist_string(),
        selected_release.title
    ));

    let album_dir = args.output_path.join(&dir_name);
    if album_dir.exists() {
        let overwrite = Confirm::new()
            .with_prompt(&format!(
                "Output path already exists: {:?}\nOverwrite?",
                album_dir
            ))
            .interact()?;
        if overwrite {
            std::fs::remove_dir_all(&album_dir)?;
        }
    }
    std::fs::create_dir_all(&album_dir)?;

    let c_disc_device = CString::new(disc_device);
    let cdrom = CdromDrive::identify(
        c_disc_device.unwrap().as_c_str(),
        cdparanoia::Verbosity::PrintIt,
    )
    .context("failed to identify CD drive")?;
    cdrom.set_verbosity(cdparanoia::Verbosity::LogIt, cdparanoia::Verbosity::LogIt);
    cdrom.open().context("failed to open CD drive")?;
    let mut paranoia = CdromParanoia::init(cdrom);
    paranoia.set_mode(ParanoiaMode::FULL);

    if let Some(error) = paranoia.drive().errors() {
        for line in error.to_string_lossy().lines() {
            println!("{}", line);
        }
    }
    if let Some(message) = paranoia.drive().messages() {
        for line in message.to_string_lossy().lines() {
            println!("{}", line);
        }
    }

    let track_count = paranoia.drive().tracks()?;
    for track_num in 1..=track_count {
        if !paranoia.drive().track_audiop(track_num)? {
            println!("WARN: Track {} is not an audio track; skipping", track_num);
            continue;
        }

        let start_time = Instant::now();

        let first_sector = paranoia.drive().track_first_sector(track_num)?;
        let last_sector = paranoia.drive().track_last_sector(track_num)?;
        let total_sectors = last_sector - first_sector + 1;
        let track_channels = paranoia.drive().track_channels(track_num)?;
        let track_duration =
            total_sectors as u32 * CD_FRAMEWORDS / (CD_SAMPLE_RATE * track_channels);

        let mb_track_info = &mb_disc_info.tracks[track_num as usize - 1];

        let file_name = if multi_disc {
            format!(
                "{}-{:02} {}.flac",
                mb_disc_info.position, track_num, mb_track_info.title
            )
        } else {
            format!("{:02} {}.flac", track_num, mb_track_info.title)
        };

        println!();
        println!(
            "Track {:02}: Ripping {} sectors ({}:{:02})",
            track_num,
            total_sectors,
            track_duration / 60,
            track_duration % 60,
        );
        println!("Output filename: {:?}", file_name);

        let mut encoder = FlacEncoder::new()
            .unwrap()
            .channels(track_channels)
            .sample_rate(CD_SAMPLE_RATE)
            .bits_per_sample(16)
            .init_file(&album_dir.join(&file_name))
            .map_err(|e| anyhow!("{:?}", e))?;

        let mut widen_buffer = [0i32; CD_FRAMEWORDS as usize];

        paranoia.seek(SeekFrom::Start(first_sector))?;
        for _ in first_sector..=last_sector {
            let sector_data = paranoia.read(event_callback);
            for (dst, src) in widen_buffer.iter_mut().zip(sector_data) {
                *dst = (*src).into();
            }
            encoder
                .process_interleaved(&widen_buffer, CD_FRAMEWORDS / track_channels)
                .map_err(|e| anyhow!("{:?}", e))?;

            if let Some(error) = paranoia.drive().errors() {
                for line in error.to_string_lossy().lines() {
                    println!("{}", line);
                }
            }
            if let Some(message) = paranoia.drive().messages() {
                for line in message.to_string_lossy().lines() {
                    println!("{}", line);
                }
            }
        }

        encoder
            .finish()
            .map_err(|enc| anyhow!("{:?}", enc.state()))?;

        let end_time = Instant::now();

        let rip_duration = (end_time - start_time).as_secs_f32();
        let speedup = track_duration as f32 / rip_duration;

        println!("Elapsed: {:.1} sec ({:.1}x)", rip_duration, speedup);
    }

    Ok(())
}

extern "C" fn event_callback(position: c_long, event: c_int) {
    let _ = (position, event); //TODO
}
