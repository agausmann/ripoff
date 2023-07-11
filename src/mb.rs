use serde::Deserialize;

const DEFAULT_ROOT_URL: &str = "https://musicbrainz.org/ws/2";
const DEFAULT_USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " ( ",
    env!("CARGO_PKG_HOMEPAGE"),
    " )",
);

pub struct Client {
    root_url: String,
    user_agent: String,
}

impl Client {
    pub fn new() -> Self {
        Self {
            root_url: DEFAULT_ROOT_URL.into(),
            user_agent: DEFAULT_USER_AGENT.into(),
        }
    }

    pub fn set_root_url(&mut self, root_url: String) {
        self.root_url = root_url
    }

    pub fn set_user_agent(&mut self, user_agent: String) {
        self.user_agent = user_agent
    }

    pub fn get(&self, path_and_query: &str) -> ureq::Request {
        ureq::get(&format!("{}/{}", self.root_url, path_and_query))
            .set("User-Agent", &self.user_agent)
            .set("Accept", "application/json")
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DiscId {
    pub id: String,
    pub releases: Vec<Release>,
}

impl DiscId {
    pub fn lookup(client: &Client, disc_id: &str) -> anyhow::Result<Self> {
        const INCLUDES: &str = "artist-credits+recordings+labels";

        let response = client
            .get(&format!("discid/{}?inc={}", disc_id, INCLUDES))
            .call()?
            .into_json()?;

        Ok(response)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Release {
    pub artist_credit: Vec<ArtistCredit>,
    pub asin: Option<String>,
    pub barcode: Option<String>,
    pub country: Option<String>,
    pub cover_art_archive: Option<CoverArtArchive>,
    pub date: String,
    pub disambiguation: String,
    pub id: String,
    pub label_info: Vec<LabelInfo>,
    pub media: Vec<Media>,
    pub packaging: String,
    pub packaging_id: String,
    pub quality: String,
    pub title: String,
}

impl Release {
    pub(crate) fn artist_string(&self) -> String {
        self.artist_credit
            .iter()
            .flat_map(|credit| [credit.name.as_str(), credit.joinphrase.as_str()])
            .collect()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ArtistCredit {
    pub artist: Artist,
    pub joinphrase: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Artist {
    pub disambiguation: String,
    pub id: String,
    pub name: String,
    pub sort_name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub type_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CoverArtArchive {
    pub artwork: bool,
    pub back: bool,
    pub count: u32,
    pub darkened: bool,
    pub front: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LabelInfo {
    pub catalog_number: String,
    pub label: Label,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Label {
    pub disambiguation: String,
    pub id: String,
    pub label_code: Option<String>,
    pub name: String,
    pub sort_name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub type_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Media {
    pub format: String,
    pub format_id: String,
    pub position: u32,
    pub title: String,
    pub track_count: u32,
    pub track_offset: u32,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Track {
    pub artist_credit: Vec<ArtistCredit>,
    pub id: String,
    pub number: String,
    pub position: u32,
    pub recording: Recording,
    pub title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Recording {
    pub artist_credit: Vec<ArtistCredit>,
    pub disambiguation: String,
    pub id: String,
    pub title: String,
}
