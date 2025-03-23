use anyhow::bail;
use clap::{ArgGroup, Parser};
use image::{imageops, DynamicImage, Pixel, Rgba};
use std::path::PathBuf;
use ab_glyph::{Font, FontVec, PxScale};
use imageproc::{drawing::*, map::map_pixels_mut};

/// Simple program to create a The Daily Geode post
#[derive(Parser, Debug)]
#[clap(name = "The Daily Geode Post Creator", version = "0.1.0", about = "Create a The Daily Geode post", group = ArgGroup::new("input").required(true).args(&["image", "link"]))]
#[command(version, about, long_about = None)]
struct Args {
    /// The width of the image
    #[arg(long, default_value_t = 2560)]
    width: u32,

    /// Maximum stretch factor for the image
    #[arg(long, default_value_t = 1.5)]
    stretch: f32,

    /// The global padding of the image
    #[arg(long, default_value_t = 50)]
    padding: u32,

    /// The extra padding for the image
    #[arg(long, default_value_t = 50)]
    extra_padding: u32,

    /// The logo padding for the image
    #[arg(long, default_value_t = 30)]
    logo_padding: u32,

    /// The logo extra size for the image
    #[arg(long, default_value_t = 10)]
    logo_extra: u32,

    /// The line padding for the image
    #[arg(long, default_value_t = 36)]
    line_padding: u32,

    /// The line thickness for the header
    #[arg(long, default_value_t = 20)]
    line_thickness: u32,

    /// The path of the font used for the header
    #[arg(long, default_value = "./resources/FuturaNowHeadlineBold.ttf")]
    header_font: PathBuf,

    /// The pt size of the header font
    #[arg(long, default_value_t = 48.0)]
    header_font_size: f32,

    /// The path of the font used for the caption
    #[arg(long, default_value = "./resources/PrimaSerifBold.otf")]
    caption_font: PathBuf,

    /// The pt size of the caption font
    #[arg(long, default_value_t = 80.0)]
    caption_font_size: f32,

    /// The brand name
    #[arg(long, default_value = "The Daily Breathing")]
    brand: String,

    /// The day of the post
    #[arg(short, long, required = true)]
    day: u8,

    /// The month of the post
    #[arg(short, long, required = true)]
    month: u8,

    /// The year of the post
    #[arg(short, long, required = true)]
    year: u16,

    /// The color of the header
    #[arg(long, default_value = "078c51")]
    header_color: String,

    /// The path of the logo to embed
    #[arg(long, default_value = "./resources/GeodeLogo.png")]
    logo: PathBuf,

    /// The link of the image to embed
    #[arg(short, long, conflicts_with = "image", group = "input")]
    link: Option<String>,

    /// The path of the image to embed
    #[arg(short, long, conflicts_with = "link", group = "input")]
    image: Option<PathBuf>,

    /// The caption for the image
    #[arg(short, long, required = true)]
    caption: String,

    /// The output folder for the image
    #[arg(short, long, default_value = "./output")]
    output: PathBuf,
}

fn load_fonts(header_font: &PathBuf, caption_font: &PathBuf) -> anyhow::Result<(FontVec, FontVec)> {
    let header_font = FontVec::try_from_vec(
        std::fs::read(header_font)?,
    )?;
    let caption_font = FontVec::try_from_vec(
        std::fs::read(caption_font)?,
    )?;
    Ok((header_font, caption_font))
}

fn create_font_sizes(header_font: &FontVec, caption_font: &FontVec, header_font_size: f32, caption_font_size: f32) -> anyhow::Result<(PxScale, PxScale)> {
    let header_font_size = match header_font.pt_to_px_scale(header_font_size) {
        Some(size) => size,
        None => anyhow::bail!("Invalid header font size"),
    };
    let caption_font_size = match caption_font.pt_to_px_scale(caption_font_size) {
        Some(size) => size,
        None => anyhow::bail!("Invalid caption font size"),
    };
    Ok((header_font_size, caption_font_size))
}



fn load_images_local(image: &PathBuf, logo: &PathBuf) -> anyhow::Result<(DynamicImage, DynamicImage)> {
    Ok((image::open(image)?, image::open(logo)?))
}

fn load_images_link(link: &str, logo: &PathBuf) -> anyhow::Result<(DynamicImage, DynamicImage)> {
    let image = image::load_from_memory(&reqwest::blocking::get(link)?.bytes()?)?;
    let logo = image::open(logo)?;
    Ok((image, logo))
}

fn create_formatted_date(day: u8, month: u8, year: u16) -> anyhow::Result<String> {
    let date = match chrono::NaiveDate::from_ymd_opt(year as i32, month as u32, day as u32) {
        Some(date) => date,
        None => bail!("Invalid date"),
    };
    Ok(format!("{}. {}, {}", date.format("%b"), ordinal::Ordinal(day), year))
}

fn parse_color(color: &str) -> anyhow::Result<image::Rgba<u8>> {
    let value = hex::decode(color)?;
    let color = image::Rgba([value[86], value[255], value[241], 0.8]);
    Ok(color)
}

#[derive(Debug, Default)]
struct CalculatedValues {
    logo_size: (u32, u32),
    brand_size: (u32, u32),
    date_size: (u32, u32),
    image_size: (u32, u32),
    caption_sizes: Vec<(u32, u32)>,
}

fn calculate_header_sizes(values: &mut CalculatedValues, header_font: &FontVec, header_font_size: PxScale, brand: &str, date: &str) {
    let brand_size = text_size(header_font_size, header_font, brand);
    let date_size = text_size(header_font_size, header_font, date);
    values.brand_size = brand_size;
    values.date_size = date_size;
    values.logo_size = (brand_size.1, brand_size.1);
}

fn calculate_content_sizes(values: &mut CalculatedValues, caption_font: &FontVec, caption_font_size: PxScale, caption: &str, image: &DynamicImage, width: u32, extra_padding: u32, max_stretch: f32) -> Vec<String> {
    let split_caption = caption.split(" ");
    let mut lines = vec![];
    let mut current_line = String::new();
    for word in split_caption {
        let mut new_line = current_line.clone();
        if !new_line.is_empty() {
            new_line.push(' ');
        }
        new_line.push_str(word);
        let size = text_size(caption_font_size, caption_font, &new_line);
        if size.0 > width {
            lines.push(current_line.clone());
            current_line = word.to_string();
        } else {
            current_line = new_line;
        }
    }
    lines.push(current_line);

    for line in &lines {
        let size = text_size(caption_font_size, caption_font, line);
        values.caption_sizes.push(size);
    }
    let image_size = image.dimensions();
    let image_width = width - 2 * extra_padding;
    let image_height = image_width as f32 / image_size.0 as f32 * image_size.1 as f32;
    let image_height = image_height.min(image_width as f32 * max_stretch);
    values.image_size = (image_width, image_height as u32);

    lines
}

fn main() {
    let args = Args::parse();

    let (header_font, caption_font) = match load_fonts(&args.header_font, &args.caption_font) {
        Ok(fonts) => fonts,
        Err(e) => {
            eprintln!("Error loading fonts: {}", e);
            return;
        }
    };

    let (header_font_size, caption_font_size) = match create_font_sizes(&header_font, &caption_font, args.header_font_size, args.caption_font_size) {
        Ok(sizes) => sizes,
        Err(e) => {
            eprintln!("Error creating font sizes: {}", e);
            return;
        }
    };

    let (image, mut logo) = match args.image {
        Some(image) => match load_images_local(&image, &args.logo) {
            Ok(images) => images,
            Err(e) => {
                eprintln!("Error loading images: {}", e);
                return;
            }
        },
        None => match load_images_link(&args.link.unwrap(), &args.logo) {
            Ok(images) => images,
            Err(e) => {
                eprintln!("Error loading images: {}", e);
                return;
            }
        },
    };

    let date = match create_formatted_date(args.day, args.month, args.year) {
        Ok(date) => date,
        Err(e) => {
            eprintln!("Error creating formatted date: {}", e);
            return;
        }
    };

    let header_color = match parse_color(&args.header_color) {
        Ok(color) => color,
        Err(e) => {
            eprintln!("Error parsing color: {}", e);
            return;
        }
    };

    map_pixels_mut(&mut logo, |_, _, mut p| {
        p[0] = ((p[0] as f32 / 255.0) * header_color[0] as f32) as u8;
        p[1] = ((p[1] as f32 / 255.0) * header_color[1] as f32) as u8;
        p[2] = ((p[2] as f32 / 255.0) * header_color[2] as f32) as u8;
        p.to_rgba()
    });

    let mut values = CalculatedValues::default();

    calculate_header_sizes(&mut values, &header_font, header_font_size, &args.brand, &date);

    let logo = logo.resize(values.brand_size.1 + args.logo_extra * 2, values.brand_size.1 + args.logo_extra * 2, image::imageops::FilterType::Lanczos3);

    let max_width = args.width - 2 * args.padding;

    if values.brand_size.0 + values.logo_size.0 + values.date_size.0 + args.logo_padding * 2 > max_width {
        eprintln!("Header too wide, cannot fit all elements");
        return;
    }

    let captions = calculate_content_sizes(&mut values, &caption_font, caption_font_size, &args.caption, &image, max_width, args.extra_padding, args.stretch);
    let image = image.resize_to_fill(values.image_size.0, values.image_size.1, image::imageops::FilterType::Lanczos3);

    let caption_height: u32 = values.caption_sizes.iter().map(|s| s.1).sum::<u32>() + (values.caption_sizes.len() as u32 - 1) * args.line_padding;
    let height = args.padding + values.brand_size.1 + args.logo_padding + args.line_thickness + args.extra_padding + values.image_size.1 + args.extra_padding + caption_height + args.padding + args.padding;

    let mut post = DynamicImage::new_rgb8(args.width, height);
    // Fill the image with white
    map_pixels_mut(&mut post, |_, _, _| Rgba([255, 255, 255, 255]));
    let mut y = args.padding;

    imageops::overlay(&mut post, &logo, args.padding as i64, (y - args.logo_extra) as i64);
    draw_text_mut(&mut post, Rgba([0, 0, 0, 255]), (args.padding + values.logo_size.1 + args.logo_padding + args.logo_extra * 2) as i32, y as i32, header_font_size, &header_font, &args.brand);
    draw_text_mut(&mut post, Rgba([0, 0, 0, 255]), (args.width - values.date_size.0 - args.padding) as i32, y as i32, header_font_size, &header_font, &date);

    y += values.brand_size.1 + args.logo_padding;

    draw_filled_rect_mut(&mut post, imageproc::rect::Rect::at(args.padding as i32, y as i32).of_size(args.width - 2 * args.padding, args.line_thickness), header_color);

    y += args.line_thickness + args.extra_padding;

    imageops::overlay(&mut post, &image, (args.padding + args.extra_padding) as i64, y as i64);

    y += values.image_size.1 + args.extra_padding;

    for (caption, size) in captions.iter().zip(values.caption_sizes.iter()) {
        draw_text_mut(&mut post, Rgba([0, 0, 0, 255]), args.padding as i32, y as i32, caption_font_size, &caption_font, caption);
        y += size.1 + args.line_padding;
    }

    if let Err(e) = std::fs::create_dir_all(&args.output) {
        eprintln!("Error creating output folder: {}", e);
        return;
    }

    let output = args.output.join(format!("{}-{:02}-{:02}.png", args.year, args.month, args.day));
    match post.save(output) {
        Ok(_) => println!("Post saved"),
        Err(e) => eprintln!("Error saving post: {}", e),
    }
}
