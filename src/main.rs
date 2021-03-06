use std::{
    error::Error,
    fs,
    io::{self, Write},
    ops::Range,
    path::{self, Path, PathBuf},
    time::Instant,
};

use image::{imageops, DynamicImage};
use rand::Rng;

#[allow(dead_code)]
enum FindType {
    File,
    Dir,
}

fn list_dir<P: AsRef<Path>>(dir: P, find_dirs: FindType) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::<PathBuf>::new();
    for item in fs::read_dir(dir)? {
        let item = item?;
        match &find_dirs {
            FindType::File => {
                if item.file_type()?.is_file() {
                    files.push(item.path());
                }
            }
            FindType::Dir => {
                if item.file_type()?.is_dir() {
                    files.push(item.path());
                }
            }
        }
    }
    Ok(files)
}

fn prompt_number(bounds: Range<u32>, message: &str, def: i32) -> io::Result<u32> {
    let stdin = io::stdin();
    let mut buffer = String::new();
    // Tell the user to enter a value within the bounds
    if message != "" {
        if def >= 0 {
            println!(
                "{} in the range [{}:{}] (default: {})",
                message,
                bounds.start,
                bounds.end - 1,
                def
            );
        } else {
            println!(
                "{} in the range [{}:{}]",
                message,
                bounds.start,
                bounds.end - 1
            );
        }
    }
    buffer.clear();
    // Keep prompting until the user passes a value within the bounds
    Ok(loop {
        stdin.read_line(&mut buffer)?;
        print!("\r\u{8}");
        io::stdout().flush().unwrap();
        if let Ok(value) = buffer.trim().parse() {
            if bounds.contains(&value) {
                break value;
            }
        } else if def >= 0 {
            print!("\r\u{8}");
            print!("{}\n", &def);
            io::stdout().flush().unwrap();
            break def as u32;
        }
        buffer.clear();
    })
}

fn input_prompt<P: AsRef<Path>>(
    dir: P,
    find_dirs: FindType,
    message: &str,
) -> std::io::Result<PathBuf> {
    // Get files/dirs in dir
    let files = list_dir(&dir, find_dirs)?;
    // Inform the user that they will need to enter a value
    if message != "" {
        println!("{}", message);
    }
    // Enumerate the names of the files/dirs
    for (i, e) in files.iter().enumerate() {
        println!("{}: {}", i, e.display());
    }
    // This is the range of values they can pick
    let bound: Range<u32> = Range {
        start: 0,
        end: files.len() as u32,
    };
    // Return the path they picked
    Ok((&files[prompt_number(bound, "", -1)? as usize]).clone())
}

fn find_compression_ratio(
    img: &DynamicImage,
    ext: &str,
) -> Result<(f64, DynamicImage), Box<dyn Error>> {
    let fstr = format!("temp{s}temp.{e}", e = &ext, s = path::MAIN_SEPARATOR);
    img.save(&fstr)?;
    let image = image::open(&fstr)?;
    Ok((
        fs::metadata(&fstr)?.len() as f64 / image.to_rgba8().to_vec().len() as f64,
        image,
    ))
}

fn find_largest_within(
    img: &DynamicImage,
    target: u64,
    ext: &str,
    iname: &str,
    m: i32,
    byte_diff: u64,
) -> Result<(), Box<dyn Error>> {
    let save_name = format!(
        "temp{s}{f}.{e}",
        f = &iname,
        e = &ext,
        s = path::MAIN_SEPARATOR
    );
    let (ratio, mut new_image) = find_compression_ratio(&img, ext)?;
    img.save(&save_name)?;
    let osize = fs::metadata(&save_name)?.len() as f64;
    let mut psize = osize;
    if psize < target as f64 {
        psize = target as f64;
    }
    let (w, h) = (img.width() as f64, img.height() as f64);
    let mut scale = 1.0;
    new_image = new_image.resize(
        (w * scale) as u32,
        (h * scale) as u32,
        imageops::FilterType::Lanczos3,
    );
    new_image.save(&save_name)?;
    // println!("Scale: {}, v: {}, OFF: {}", scale, v, (1.50001 * (1.0 - v)) + v);
    let mut i = 0;
    let mut diff_ratio = 0f64;
    let mut diff = 0f64;
    let mut imgsize = psize;
    let mut rng = rand::thread_rng();
    let mut best_scale = 1.0f64;
    let mut best_diff = f64::MAX;
    let mut best_size = imgsize;
    let (mut a, mut b) = (0.0f64, 1.0f64);
    if target > osize as u64 {
        a = b;
        b = (target as f64 / osize) as f64 * 1.05;
    }

    println!("Starting!");
    while (diff.abs() > byte_diff as f64 || diff_ratio != 1.0 || diff_ratio > 1.0)
        || i == 0
        || fs::metadata(&save_name)?.len() as f64 > target as f64
    {
        imgsize = fs::metadata(&save_name)?.len() as f64;
        diff = imgsize as f64 - target as f64;
        diff_ratio = (imgsize as f64 * ratio) / target as f64;

        if diff.abs() < best_diff.abs() && diff < 0.0 {
            best_scale = scale;
            best_diff = diff;
            best_size = imgsize;
            println!("\r\u{8}||{:^wa$}({:6.2}%) || BEST_DIFF: {:>width$}, BEST_SCALE: {:5.2} || SCALE: {:.2} || RANGE: ({:>5.2}:{:<5.2}) ||", i, (i as f32 / m as f32) * 100.0, best_diff, best_scale, scale, a, b, wa=(m.to_string().len()+2), width=(psize.to_string().len()));
        }

        if i > m || (1.0 - (a.min(b) / a.max(b))).abs() < 0.05 || diff.abs() < byte_diff as f64 {
            break;
        } else {
            print!("\r\u{8}");
            print!(
                "||{:^wa$}({:3.2}%) ||",
                i,
                (i as f32 / m as f32) * 100.0,
                wa = (m.to_string().len() + 2)
            );
            io::stdout().flush().unwrap();
        }

        let lscale = scale;
        if imgsize < target as f64 {
            a = scale - (1.0 / (i + 2) as f64);
        } else {
            b = scale + (1.0 / (i + 2) as f64);
        }
        if diff_ratio < 1.0 {
            scale = rng.gen_range(a..b) as f64;
        } else {
            scale = rng.gen_range(a..b) as f64;
        }
        if scale < 0.0 {
            scale = lscale;
        }
        let (w, h) = (img.width() as f64, img.height() as f64);
        new_image = img.resize(
            (w * scale) as u32,
            (h * scale) as u32,
            imageops::FilterType::Lanczos3,
        );
        new_image.save(&save_name)?;

        i += 1;
    }
    println!(
        "\rStopped at ||{:^wa$}({:3.2}%) ||",
        i,
        (i as f32 / m as f32) * 100.0,
        wa = (m.to_string().len() + 2)
    );
    let (w, h) = (img.width() as f64, img.height() as f64);
    new_image = img.resize(
        (w * best_scale) as u32,
        (h * best_scale) as u32,
        imageops::FilterType::Lanczos3,
    );
    let mut datatype = "B";
    match best_size as u64 {
        1_000..=999_999 => {
            datatype = "KB";
        }
        1_000_000..=999_999_999 => {
            datatype = "MB";
        }
        1_000_000_000..=999_999_999_999 => {
            datatype = "GB";
        }
        /* ??Could you IMAGINE?? */
        1_000_000_000_000..=999_999_999_999_999 => {
            datatype = "TB";
        }
        _ => (),
    }
    // idk, couldn't change best_size in the above match, lmao
    let mut best_size_out = best_size as u64;
    match datatype {
        "KB" => best_size_out = best_size as u64 / 1_000,
        "MB" => best_size_out = best_size as u64 / 1_000_000,
        "GB" => best_size_out = best_size as u64 / 1_000_000_000,
        "TB" => best_size_out = best_size as u64 / 1_000_000_000_000,
        _ => (),
    }
    new_image.save(format!(
        "{f}_{s}{t}.{e}",
        f = &iname,
        e = &ext,
        s = best_size_out,
        t = datatype
    ))?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let fname = input_prompt("input", FindType::File, "Please select an image: ")?;
    let ps = fname.file_name().unwrap().to_string_lossy();
    let ext = String::from(ps.split(".").collect::<Vec<&str>>()[1]);
    let oname = String::from(ps.split(".").collect::<Vec<&str>>()[0]);
    let image = image::open(&fname)?;
    let target: u64 = prompt_number(
        Range {
            start: 128,
            end: u32::MAX,
        },
        "\nEnter desired filesize in bytes\nChoose a value",
        1000,
    )? as u64;
    let byte_halt = prompt_number(
        Range {
            start: 0,
            end: u32::MAX,
        },
        "\nEnter the byte threshold (stop when the diff is equal or less than this)\n(It may not be possible to exactly reach the filesize)\nChoose a value",
        128
    )? as u64;
    let iters = prompt_number(
        Range {
            start: 8,
            end: 16384,
        },
        "\nEnter number of iterations to run (more = closer filesize to target)\nChoose a value",
        256,
    )? as i32;
    println!("\nOk! One moment...");
    let now = Instant::now();
    find_largest_within(&image, target, &ext, &oname, iters, byte_halt)?;
    println!("\nFinished in: {}ms!", now.elapsed().as_millis());
    Ok(())
}
