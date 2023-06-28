use clap::Parser;
use discid::DiscId;

#[derive(clap::Parser)]
pub struct Cli {
    disc_device: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let disc_info = DiscId::read(args.disc_device.as_deref())?;
    let disc_id = disc_info.id();
    let toc = disc_info.toc_string();

    println!("Disc ID: {:?}", disc_id);
    println!("TOC: {:?}", toc);

    const USER_AGENT: &str = concat!(
        env!("CARGO_PKG_NAME"),
        "/",
        env!("CARGO_PKG_VERSION"),
        " ( ",
        env!("CARGO_PKG_HOMEPAGE"),
        " )",
    );
    let response = ureq::get(&format!("https://musicbrainz.org/ws/2/discid/{}", disc_id))
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/json")
        .call()?
        .into_string()?;

    println!("{}", response);

    Ok(())
}
