use std::{env, fs::File, io::BufWriter, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(&env::var("OUT_DIR")?).join("codegen.rs");
    let mut file = BufWriter::new(File::create(path)?);

    regex_build::write_regex(
        "ATTRIBUTE_RE", // helps with splitting name="value"
        r#"[a-zA-Z_][0-9a-zA-Z\-_]+\s*=\s*(
            ("(?:\\"|[^"])*[^\\]")| # name="value"
            ('(?:\\'|[^'])*[^\\]')| # name='value'
            ([^'"](?:\\\s|[^\s>]*)) # name=value or name=value>
        )"#,
        &mut file,
    )?;
    regex_build::write_regex(
        "META_TAGS", // identifies HTML tags valid for metadata
        r"<(?i)( # NOTE: Tags are case-insensitive
            meta\x20|                   # Regular meta tags
            title[^>]*>|                # <title> element, skipping over attributes
            link\x20|                   # link elements
            ((div|span)[^>]+itemscope)  # itemscopes
        )",
        &mut file,
    )?;

    Ok(())
}
