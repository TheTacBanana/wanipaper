use image::{ImageReader, RgbaImage};
use serde::Deserialize;
use std::{collections::HashMap, fmt::Pointer, fs::File, path::PathBuf};
use toml::{Table, Value};

#[derive(Debug, Default)]
pub struct Config {
    pub images: HashMap<String, LoadedImage>,
    pub displays: HashMap<String, DisplayTarget>,
    pub groups: HashMap<String, DisplayGroup>,
    pub render_passes: Vec<RenderPass>,
}

#[derive(Debug)]
pub struct LoadedImage {
    pub image: RgbaImage,
}

#[derive(Debug)]
pub struct DisplayTarget {
    pub name: String,
}

#[derive(Debug)]
pub struct DisplayGroup {
    pub displays: Vec<String>,
}

#[derive(Debug)]
pub struct RenderPass {
    pub source: RenderSource,
    pub target: RenderTarget,
    pub resize: ResizeKind,
}

#[derive(Debug)]
pub enum RenderSource {
    Single(String),
    // Many(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RenderTarget {
    Display(String),
    Group(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResizeKind {
    #[default]
    None,
    Cover,
    Stretch,
}

pub enum ConfigError {
    MissingConfig(PathBuf),

    Io(std::io::Error),
    Toml(toml::de::Error),
    Image(image::error::ImageError),
    UnknownKey(String),

    /// Ident is both a display and a group
    AmbiguousRenderTarget(String),
    /// Render target cannot be found
    UnknownRenderTarget(String),

    /// Image could not be found
    UnknownImage(String),
    /// Display could not be found
    UnknownDisplay(String),
    /// Group could not be found
    UnknownGroup(String),
}

pub enum Category {
    Image,
    Display,
    Group,
    RenderPass,
}

impl Config {
    pub fn load() -> Result<Config, ConfigError> {
        let mut config_path = std::env::home_dir().expect("Failed to get home directory");
        config_path.push(".config/wani/");

        std::fs::create_dir_all(&config_path).expect("Failed to create config directory");
        config_path.push("wanipaper.config");

        if !config_path.exists() {
            return Err(ConfigError::MissingConfig(config_path.clone()));
        }

        // Read config file
        let config_file = std::fs::read_to_string(config_path).map_err(ConfigError::Io)?;

        // Parse into toml
        let mut table: Table = toml::from_str(&config_file).map_err(ConfigError::Toml)?;

        // Create default config
        let mut config = Config::default();

        // Load Images
        {
            #[derive(Deserialize)]
            struct ImageConfig {
                path: String,
            }

            if let Some(Value::Table(images)) = table.remove("images") {
                for (ident, image) in images {
                    let image: ImageConfig = image.try_into().map_err(ConfigError::Toml)?;

                    let image = ImageReader::open(image.path)
                        .map_err(ConfigError::Io)?
                        .with_guessed_format()
                        .map_err(ConfigError::Io)?
                        .decode()
                        .map_err(ConfigError::Image)?
                        .into_rgba8();

                    config.images.insert(ident, LoadedImage { image });
                }
            };
        }

        // Load Displays
        {
            #[derive(Deserialize)]
            struct DisplayConfig {
                name: String,
            }

            if let Some(Value::Table(displays)) = table.remove("displays") {
                for (ident, display) in displays {
                    let display: DisplayConfig = display.try_into().map_err(ConfigError::Toml)?;

                    config
                        .displays
                        .insert(ident, DisplayTarget { name: display.name });
                }
            }
        }

        // Load Groups
        {
            #[derive(Deserialize)]
            struct GroupConfig {
                displays: Vec<String>,
            }

            if let Some(Value::Table(groups)) = table.remove("groups") {
                for (ident, group) in groups {
                    let group: GroupConfig = group.try_into().map_err(ConfigError::Toml)?;

                    for display in &group.displays {
                        if !config.displays.contains_key(display) {
                            return Err(ConfigError::UnknownDisplay(display.clone()));
                        }
                    }

                    config.groups.insert(
                        ident,
                        DisplayGroup {
                            displays: group.displays,
                        },
                    );
                }
            }
        }

        // Load Render Passes
        {
            #[derive(Debug, Deserialize)]
            pub struct RenderConfig {
                pub source: OneOrMany<String>,
                pub target: String,
                #[serde(default)]
                pub resize: ResizeKind,
            }

            #[derive(Clone, Debug, Deserialize, PartialEq)]
            #[serde(untagged)]
            pub enum OneOrMany<T> {
                /// Single value
                One(T),
                /// Array of values
                Vec(Vec<T>),
            }

            if let Some(Value::Array(render_passes)) = table.remove("renderpass") {
                for render_pass in render_passes {
                    let render_pass: RenderConfig =
                        render_pass.try_into().map_err(ConfigError::Toml)?;

                    let source = match render_pass.source {
                        OneOrMany::One(image) => {
                            if !config.images.contains_key(&image) {
                                return Err(ConfigError::UnknownImage(image));
                            }
                            RenderSource::Single(image)
                        }
                        OneOrMany::Vec(_) => {
                            todo!()
                        }
                    };

                    let target = {
                        let valid_display = config.displays.contains_key(&render_pass.target);
                        let valid_group = config.groups.contains_key(&render_pass.target);

                        if valid_display && valid_group {
                            return Err(ConfigError::AmbiguousRenderTarget(render_pass.target));
                        } else if valid_display {
                            RenderTarget::Display(render_pass.target)
                        } else if valid_group {
                            RenderTarget::Group(render_pass.target)
                        } else {
                            return Err(ConfigError::UnknownRenderTarget(render_pass.target));
                        }
                    };

                    config.render_passes.push(RenderPass {
                        source,
                        target,
                        resize: render_pass.resize,
                    });
                }
            }
        }

        if let Some((key, _)) = table.into_iter().next() {
            return Err(ConfigError::UnknownKey(key));
        }

        return Ok(config);
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::MissingConfig(p) => write!(f, "missing config: {p:?}"),
            ConfigError::Toml(e) => write!(f, "{e}"),
            ConfigError::Io(e) => write!(f, "io error: {e}"),
            ConfigError::Image(e) => write!(f, "image error: {e}"),
            ConfigError::UnknownKey(k) => write!(f, "unknown key: {k}"),
            ConfigError::AmbiguousRenderTarget(s) => write!(f, "render yarget '{s}' is ambiguous"),
            ConfigError::UnknownRenderTarget(s) => write!(f, "'{s}' is neither a Display or Group"),
            ConfigError::UnknownImage(i) => write!(f, "image '{i}' could not be found"),
            ConfigError::UnknownDisplay(d) => write!(f, "display '{d}' could not be found"),
            ConfigError::UnknownGroup(g) => write!(f, "group '{g}' could not be found"),
        }
    }
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::Image => write!(f, "image"),
            Category::Display => write!(f, "display"),
            Category::Group => write!(f, "group"),
            Category::RenderPass => write!(f, "render pass"),
        }
    }
}
