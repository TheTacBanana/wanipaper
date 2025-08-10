use image::{ImageReader, RgbaImage};
use log::{error, info, warn};
use rand::random_range;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
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
    Many {
        images: Vec<String>,
        rand: bool,
        rotate: Option<usize>,
    },
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
    Cover,
    Stretch,
}

pub enum ConfigError {
    MissingConfig(PathBuf),

    Io(std::io::Error),
    Toml(toml::de::Error),
    Image(image::error::ImageError),

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

    /// No Render Passes
    NoRenderPasses,
}

impl Config {
    pub fn load() -> Result<Config, ConfigError> {
        // Create path to config directory
        let mut wani_path = std::env::home_dir().expect("Failed to get home directory");
        wani_path.push(".config/wani");
        std::fs::create_dir_all(&wani_path).expect("Failed to create config directory");
        info!("wanipaper directory {:?}", wani_path);

        let mut config_path = wani_path.clone();
        config_path.push("wanipaper.config");
        info!("wanipaper config path {:?}", config_path);

        if !config_path.exists() {
            return Err(ConfigError::MissingConfig(config_path.clone()));
        }

        info!("load path {:?}", config_path);

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
                    let image_config: ImageConfig = image.try_into().map_err(ConfigError::Toml)?;

                    let mut image_path = wani_path.clone();
                    image_path.push(image_config.path);
                    info!("load image {:?}", image_path);

                    let loaded_image = || -> Result<RgbaImage, ConfigError> {
                        Ok(ImageReader::open(&image_path)
                            .map_err(ConfigError::Io)?
                            .with_guessed_format()
                            .map_err(ConfigError::Io)?
                            .decode()
                            .map_err(ConfigError::Image)?
                            .into_rgba8())
                    }();

                    match loaded_image {
                        Ok(image) => {
                            config.images.insert(ident, LoadedImage { image });
                        }
                        Err(e) => {
                            error!("failed to load image: {:?}", &image_path);
                            error!("{e}");
                        }
                    }
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
                    let mut group: GroupConfig = group.try_into().map_err(ConfigError::Toml)?;

                    group.displays = group
                        .displays
                        .into_iter()
                        .filter(|display| {
                            if !config.displays.contains_key(display) {
                                warn!(
                                    "display '{display}' not found, removed from group '{ident}'"
                                );
                                return false;
                            }
                            true
                        })
                        .collect();

                    if group.displays.is_empty() {
                        error!("group '{ident}' contains no displays, group removed");
                        continue;
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
                source: OneOrMany<String>,
                #[serde(default)]
                selection: Option<SelectionConfig>,
                target: String,
                #[serde(default)]
                resize: ResizeKind,
            }

            #[derive(Clone, Debug, Deserialize, PartialEq)]
            #[serde(untagged)]
            enum OneOrMany<T> {
                /// Single value
                One(T),
                /// Array of values
                Vec(Vec<T>),
            }

            #[derive(Debug, Deserialize)]
            struct SelectionConfig {
                #[serde(default)]
                rand: bool,
                #[serde(default)]
                rotate: Option<usize>,
            }

            if let Some(Value::Array(render_passes)) = table.remove("renderpass") {
                for render_pass in render_passes {
                    let render_pass: RenderConfig =
                        render_pass.try_into().map_err(ConfigError::Toml)?;

                    let source = match render_pass.source {
                        OneOrMany::One(image) => {
                            if !config.images.contains_key(&image) {
                                error!("image '{image}' not found, removing renderpass");
                                continue;
                            }
                            RenderSource::Single(image)
                        }
                        OneOrMany::Vec(mut images) => {
                            images = images
                                .into_iter()
                                .filter(|image| {
                                    if !config.images.contains_key(image) {
                                        warn!("image '{image}' not found, removing renderpass");
                                        return false;
                                    }
                                    true
                                })
                                .collect();

                            if images.is_empty() {
                                error!("renderpass contains no sources, removed");
                                continue;
                            }

                            match render_pass.selection {
                                None => RenderSource::Single(images[0].clone()),
                                Some(SelectionConfig { rand, rotate }) => {
                                    if rotate.is_none() && rand {
                                        let image = images[random_range(0..images.len())].clone();
                                        info!("selected random image '{image}'");
                                        RenderSource::Single(image)
                                    } else {
                                        RenderSource::Many {
                                            images,
                                            rand,
                                            rotate,
                                        }
                                    }
                                }
                            }
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

        // Report unknown keys
        for (key, _) in table.into_iter() {
            error!("unknown key: '{key}'");
        }

        // Error if no render passes
        if config.render_passes.is_empty() {
            return Err(ConfigError::NoRenderPasses);
        }

        // Remove unused images
        {
            let mut image_uses = HashSet::new();
            for pass in config.render_passes.iter() {
                match &pass.source {
                    RenderSource::Single(image) => {
                        image_uses.insert(image);
                    }
                    RenderSource::Many { images, .. } => {
                        for image in images {
                            image_uses.insert(image);
                        }
                    }
                }
            }
            config.images = config
                .images
                .into_iter()
                .filter(|(ident, _)| {
                    if image_uses.contains(ident) {
                        true
                    } else {
                        info!("image '{ident}' unused, removed");
                        false
                    }
                })
                .collect();
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
            ConfigError::AmbiguousRenderTarget(s) => write!(f, "render yarget '{s}' is ambiguous"),
            ConfigError::UnknownRenderTarget(s) => write!(f, "'{s}' is neither a Display or Group"),
            ConfigError::UnknownImage(i) => write!(f, "image '{i}' could not be found"),
            ConfigError::UnknownDisplay(d) => write!(f, "display '{d}' could not be found"),
            ConfigError::UnknownGroup(g) => write!(f, "group '{g}' could not be found"),
            ConfigError::NoRenderPasses => write!(f, "no render passes could be found"),
        }
    }
}
