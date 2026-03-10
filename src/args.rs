use std::str::FromStr;

use crate::api::Url;
use clap::{ArgGroup, Args, Parser, Subcommand};

#[derive(Parser)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
//#[derive(Debug, Subcommand, Clone)]
pub enum Command {
    Sync,
    Add {
        #[arg(required = true)]
        paths: Vec<String>,
    },
    Remove {
        #[arg(required = true)]
        ids: Vec<String>,
    },
    List,
    Clean,
    /// Get wallpaper info (supports both local and API lookup)
    Info {
        #[arg(required = true)]
        id: String,
    },
    /// Search wallpaper by query or colors
    Search(SearchArgs),
    /// Get tag info
    TagInfo(TagInfoArgs),
    /// Show user settings
    UserSettings(UserSettingsArgs),
    /// Show user collections
    UserCollections(UserCollectionsArgs),
}

#[derive(Debug, Args)]
#[clap(group(
            ArgGroup::new("search_method")
                .required(true)
                .multiple(false)
                .args(&["query", "colors"]),
        ))]
pub struct SearchArgs {
    /// Download wallpapers to save_location from config (uses Wallhaven ID as filename)
    #[clap(short = 'd', long, verbatim_doc_comment, help_heading = "DOWNLOAD")]
    pub download: bool,

    /// Query string
    ///
    ///    tagname - search fuzzily for a tag/keyword
    ///    -tagname - exclude a tag/keyword
    ///    +tag1 +tag2 - must have tag1 and tag2
    ///    +tag1 -tag2 - must have tag1 and NOT tag2
    ///    @username - user uploads
    ///    id:123 - Exact tag search (can not be combined)
    ///    type:{png/jpg} - Search for file type (jpg = jpeg)
    ///    like:wallpaper ID - Find wallpapers with similar tags
    ///
    ///    Ex. "anime +city -mountain type:png"
    #[clap(short = 'q',
           long,
           verbatim_doc_comment,
           help_heading = Some("SEARCH"),
           //default_value = "",
           value_parser = clap::value_parser!(SearchQuery),
    )]
    query: Option<SearchQuery>,

    /// Categories
    ///
    ///    Turn categories on(1) or off(0)
    ///    (general/anime/people).
    #[clap(short = 'c',
           long,
           verbatim_doc_comment,
           help_heading = Some("SEARCH PREFERENCES"),
           //default_value = "111",
           value_parser = ["100", "101", "110", "111"],
    )]
    categories: Option<String>,

    /// Purity
    ///
    ///    Turn purities on(1) or off(0)
    ///    *NSFW requires a valid API key*
    ///    (sfw/sketchy/nsfw).
    #[clap(short = 'p',
           long,
           verbatim_doc_comment,
           help_heading = Some("SEARCH PREFERENCES"),
           //default_value = "100",
           value_parser = ["100", "101", "110", "111"],
    )]
    purity: Option<String>,

    /// Sorting
    ///
    #[clap(short = 's',
           long,
           verbatim_doc_comment,
           help_heading = Some("SORTING PREFERENCES"),
           ignore_case = true,
           //default_value = "DATE_ADDED",
           value_parser = ["DATE_ADDED", "RELEVANCE", "RANDOM", "VIEWS", "FAVORITES", "TOPLIST"],
    )]
    sorting: Option<String>,

    /// Sorting order
    ///
    #[clap(short = 'o',
           long,
           verbatim_doc_comment,
           help_heading = Some("SORTING PREFERENCES"),
           ignore_case = true,
           //default_value = "DESC",
           value_parser = ["ASC", "DESC"],
    )]
    order: Option<String>,

    /// Range of search
    ///
    ///    Sorting MUST be set to 'TOPLIST'
    #[clap(short = 't',
           long,
           verbatim_doc_comment,
           help_heading = Some("SORTING PREFERENCES"),
           ignore_case = true,
           //default_value = "1M",
           value_parser = ["1D", "3D", "1W", "1M", "3M", "6M", "1Y"],
    )]
    toprange: Option<String>,

    /// Atleast
    ///
    ///    Set The minimum resolution allowed
    ///    Ex. 1920x1080.
    #[clap(short = 'a',
           long,
           verbatim_doc_comment,
           help_heading = Some("WALLPAPER PREFERENCES"),
           //default_value = "",
    )]
    atleast: Option<String>,

    /// Resolutions
    ///
    ///    List of exact wallpaper resolutions
    ///    Single resolution allowed.
    #[clap(short = 'r',
           long,
           verbatim_doc_comment,
           help_heading = Some("WALLPAPER PREFERENCES"),
           //default_value = "1920x1080,1920x1200",
    )]
    resolutions: Option<String>,

    /// Ratios
    ///
    ///    List of aspect ratios
    ///    Single ratio allowed.
    ///
    ///    Ex. 16x9,16x10
    #[clap(short = 'R',
           long,
           verbatim_doc_comment,
           help_heading = Some("WALLPAPER PREFERENCES"),
           //default_value = "16x9,16x10",
    )]
    ratios: Option<String>,

    /// Color
    ///
    ///    Search by hex color
    ///    Ex.  --colors 0066cc
    ///         --colors #333393
    #[clap(short = 'C',
           long,
           verbatim_doc_comment,
           help_heading = Some("SEARCH"),
           //required_unless_present_any = ["query"],
           //conflicts_with = "query",
           //default_value = "000000",
           value_parser = valid_color
    )]
    colors: Option<String>,

    /// Page
    ///
    ///    Select page of results
    ///    (1..)
    #[clap(short = 'P',
           long,
           verbatim_doc_comment,
           help_heading = Some("SEARCH PREFERENCES"),
           //default_value_t = 1,
           value_parser = clap::value_parser!(u32).range(1..),
    )]
    page: Option<u32>,

    /// Seed
    ///
    /// Optional seed for random results (6 alphanumeric characters) [a-zA-Z0-9]{6}
    #[clap(long,
           verbatim_doc_comment,
           help_heading = Some("SEARCH PREFERENCES"),
           //default_value_t = 1,
           value_parser = clap::value_parser!(Seed),
    )]
    seed: Option<Seed>,
}

fn valid_color(s: &str) -> Result<String, String> {
    let s = if s.starts_with("#") { &s[1..] } else { s };

    let valid_hex = s.chars().into_iter().all(|c| c.is_digit(16));

    if valid_hex && s.len() == 6 {
        return Ok(String::from(s));
    } else {
        return Err(String::from(format!("{s} is not a valid hex color")));
    }
}

fn valid_wallpaper_id(s: &str) -> Result<String, String> {
    let valid_format = s
        .chars()
        .into_iter()
        .all(|c| c.is_ascii_digit() || c.is_ascii_alphabetic());

    if valid_format && s.len() == 6 {
        return Ok(String::from(s));
    } else {
        return Err(String::from(format!("{s} is not a valid wallpaper id")));
    }
}

#[derive(Debug, Args)]
pub struct TagInfoArgs {
    /// ID of tag
    pub id: i32,
}

#[derive(Debug, Args)]
pub struct UserSettingsArgs;

#[derive(Debug, Args)]
pub struct UserCollectionsArgs {
    /// Username
    ///
    /// Get username public collections
    /// If no username provided, gets all api key account collections
    username: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct SearchQuery {
    tags: Option<Vec<String>>,
    username: Option<String>,
    id: Option<String>,       //Cant be combined
    filetype: Option<String>, //type:{png/jpg}
    like: Option<String>,
}

impl FromStr for SearchQuery {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut q = Self::default();
        let mut tags = Vec::<String>::default();

        for token in s.split(" ") {
            // Get parameters key:value
            let t: Vec<&str> = token.split(":").collect();
            if t.len() == 2 {
                let key = t[0];
                let value = t[1];

                match key {
                    "id" => {
                        // Exclusive parameter
                        // Id is a tag number
                        // Maybe i should force a casting, even if api is resilient to non integer id?
                        let mut q = Self::default();
                        q.id = Some(String::from(value));
                        return Ok(q);
                    }
                    "type" => {
                        if value == "png" || value == "jpg" {
                            q.filetype = Some(String::from(value));
                        } else {
                            return Err(String::from("Invalid file type - only accept png or jpg"));
                        }
                    }
                    "like" => {
                        // Wallpaper ID
                        // As of now ID is length 6 alphanumerical String
                        // Maybe should enforce it by explicit check, and return err
                        q.like = Some(String::from(value));
                    }
                    _ => {
                        return Err(String::from(format!("{key}:{value} is not a valid query")));
                    }
                }

                continue;
            }

            // Get username if any
            if token.starts_with("@") {
                q.username = Some(String::from(&token[1..]));
                continue;
            }

            // Get tags - merging fuzzily with + and - tags
            tags.push(token.to_string());
        }

        if !tags.is_empty() {
            q.tags = Some(tags)
        }

        return Ok(q);
    }
}

#[derive(Debug, Default, Clone)]
pub struct Seed(String);

impl FromStr for Seed {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() == 6
            && s.chars()
                .all(|c| c.is_ascii_alphabetic() || c.is_ascii_digit())
        {
            return Ok(Seed(String::from(s)));
        } else {
            return Err(format!("{s} is an invalid seed"));
        }
    }
}

impl Url for SearchArgs {
    fn to_url(&self, base_url: &str) -> String {
        let mut params = Vec::<String>::new();

        // Search
        if let Some(colors) = &self.colors {
            params.push(format!("colors={colors}"))
        } else {
            if let Some(q) = &self.query {
                if let Some(id) = &q.id {
                    params.push(format!("q=id:{id}"));
                } else {
                    let mut query = Vec::<String>::new();

                    if let Some(tags) = &q.tags {
                        query.push(tags.join(" "));
                    }
                    if let Some(username) = &q.username {
                        query.push(format!("@{username}"));
                    }
                    if let Some(ft) = &q.filetype {
                        query.push(format!("type:{ft}"));
                    }
                    if let Some(w) = &q.like {
                        query.push(format!("like:{w}"));
                    }

                    params.push(format!("q={}", query.join(" ")));
                }
            }
        }

        // Search preferences
        if let Some(categories) = &self.categories {
            params.push(format!("categories={}", categories));
        }
        if let Some(purity) = &self.purity {
            params.push(format!("purity={}", purity));
        }
        if let Some(page) = self.page {
            params.push(format!("page={}", page));
        }
        if let Some(seed) = &self.seed {
            params.push(format!("seed={}", seed.0));
        }

        // Sorting
        if let Some(order) = &self.order {
            params.push(format!("order={}", order.to_ascii_lowercase()));
        }
        if let Some(sorting) = &self.sorting {
            params.push(format!("sorting={}", sorting.to_ascii_lowercase()));
        }
        if let Some(toprange) = &self.toprange {
            params.push(format!("topRange={}", toprange.to_ascii_lowercase()));
        }

        // Wallpaper Preferences
        if let Some(atleast) = &self.atleast {
            params.push(format!("atleast={}", atleast.to_ascii_lowercase()));
        }
        if let Some(resolutions) = &self.resolutions {
            params.push(format!("resolutions={}", resolutions.to_ascii_lowercase()));
        }
        if let Some(ratios) = &self.ratios {
            params.push(format!("ratios={}", ratios.to_ascii_lowercase()));
        }

        return format!("{base_url}/search?{}", params.join("&"));
    }
}

impl Url for TagInfoArgs {
    fn to_url(&self, base_url: &str) -> String {
        return format!("{base_url}/tag/{}", self.id);
    }
}

impl Url for UserSettingsArgs {
    fn to_url(&self, base_url: &str) -> String {
        return format!("{base_url}/settings");
    }
}

impl Url for UserCollectionsArgs {
    fn to_url(&self, base_url: &str) -> String {
        match &self.username {
            Some(username) => {
                return format!("{base_url}/collections/{username}");
            }
            None => {
                return format!("{base_url}/collections");
            }
        }
    }
}
