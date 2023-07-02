pub mod mb;

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

    let mb_client = mb::Client::new();
    let mb_info = mb::DiscId::lookup(&mb_client, &disc_id)?;
    println!("{:#?}", mb_info);

    Ok(())
}
