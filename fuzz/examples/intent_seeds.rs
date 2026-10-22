use std::{error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let directory = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("expected output directory")?,
    );
    fs::create_dir_all(&directory)?;
    for (i, expression) in ["", "(a|b)?c", "(ｶ|ﾊ)ﾞ", "😀?", "(()|a)", "a??", "(a|b)"]
        .iter()
        .enumerate()
    {
        let bytes: Vec<_> = expression
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        fs::write(directory.join(format!("seed-{i}")), bytes)?;
    }
    Ok(())
}
