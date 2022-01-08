use anyhow::{Context, Result};
use clap::Parser;
use css_color_parser::Color as CssColor;
use font_loader::system_fonts;
use log::{info, warn};

use crate::utils;

/// Load a system font.
fn load_font(font_family: &str) -> Result<Vec<u8>> {
    let mut font_family_property = system_fonts::FontPropertyBuilder::new()
        .family(font_family)
        .build();
    let info = system_fonts::query_specific(&mut font_family_property);
    info!("Returned effective font is: {:?}", info);
    let (loaded_font, _) =
        if let Some((loaded_font, index)) = system_fonts::get(&font_family_property) {
            (loaded_font, index)
        } else {
            warn!("Family not found, falling back to first Monospace font");
            let mut font_monospace_property =
                system_fonts::FontPropertyBuilder::new().monospace().build();
            let sysfonts = system_fonts::query_specific(&mut font_monospace_property);
            warn!("Falling back to font '{font}'", font = sysfonts[0]);
            let (loaded_font, index) = system_fonts::get(&font_monospace_property)
                .context("Couldn't find suitable font")?;
            (loaded_font, index)
        };
    Ok(loaded_font)
}

/// Generate a valid `FontConfig` from `f`.
/// `f` is expected to be in format `Mono:72`.
fn parse_truetype_font(f: &str) -> Result<FontConfig> {
    let v: Vec<_> = f.split(':').collect();
    let (family, size) = (
        v.get(0).context("Wrong font format")?,
        v.get(1).context("Wrong font format")?,
    );

    let loaded_font = load_font(family).context("Couldn't load font")?;
    let font_config = FontConfig {
        font_family: family.to_string(),
        font_size: size.parse::<f64>().context("Couldn't parse font size")?,
        loaded_font,
    };
    Ok(font_config)
}

/// Parse a color into a tuple of floats.
fn parse_color(color_str: &str) -> Result<(f64, f64, f64, f64), String> {
    let color = color_str
        .parse::<CssColor>()
        .map_err(|_| "Invalid color format")?;
    Ok((
        f64::from(color.r) / 255.0,
        f64::from(color.g) / 255.0,
        f64::from(color.b) / 255.0,
        f64::from(color.a),
    ))
}

#[derive(Debug)]
pub struct FontConfig {
    pub font_family: String,
    pub font_size: f64,
    pub loaded_font: Vec<u8>,
}

fn parse_exit_keys(s: &str) -> utils::Sequence {
    utils::Sequence::new(Some(s))
}

#[derive(Parser, Debug)]
#[clap(name = "wmfocus", author, about, version)]
pub struct AppConfig {
    /// Use a specific TrueType font with this format: family:size
    #[clap(short, long, default_value = "Mono:72", parse(try_from_str = parse_truetype_font))]
    pub font: FontConfig,

    /// Define a set of possbile values to use as hint characters
    #[clap(short = 'c', long = "chars", default_value = "asdfjkl")]
    pub hint_chars: String,

    /// Add an additional margin around the text box (value is a factor of the box size)
    #[clap(short, long, default_value = "0.2")]
    pub margin: f32,

    /// Text color (CSS notation)
    #[clap(long = "textcolor", display_order = 49, default_value = "#dddddd", parse(try_from_str = parse_color))]
    pub text_color: (f64, f64, f64, f64),

    /// Text color alternate (CSS notation)
    #[clap(long = "textcoloralt", display_order = 50, default_value = "#666666", parse(try_from_str = parse_color))]
    pub text_color_alt: (f64, f64, f64, f64),

    /// Background color (CSS notation)
    #[clap(long = "bgcolor", display_order = 51, default_value = "rgba(30, 30, 30, 0.9)", parse(try_from_str = parse_color))]
    pub bg_color: (f64, f64, f64, f64),

    /// Text color current window (CSS notation)
    #[clap(long = "textcolorcurrent", display_order = 52, default_value = "#333333", parse(try_from_str = parse_color))]
    pub text_color_current: (f64, f64, f64, f64),

    /// Text color current window alternate (CSS notation)
    #[clap(long = "textcolorcurrentalt", display_order = 53, default_value = "#999999", parse(try_from_str = parse_color))]
    pub text_color_current_alt: (f64, f64, f64, f64),

    /// Background color current window (CSS notation)
    #[clap(long = "bgcolorcurrent", display_order = 54, default_value = "rgba(200, 200, 200, 0.9)", parse(try_from_str = parse_color))]
    pub bg_color_current: (f64, f64, f64, f64),

    /// List of keys to exit application, sequences separator is space, key separator is '+', eg Control_L+g Shift_L+f
    #[clap(short, long, parse(from_str = parse_exit_keys))]
    pub exit_keys: Vec<utils::Sequence>,
}

pub fn parse_args() -> AppConfig {
    let config = AppConfig::parse();
    config
}
