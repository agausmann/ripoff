use clap::Parser;
use discid::DiscId;

#[derive(clap::Parser)]
pub struct Cli {
    disc_device: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let disc_id = DiscId::read(args.disc_device.as_deref())?;

    println!("{:?}", disc_id.id());

    Ok(())
}
