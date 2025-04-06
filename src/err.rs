pub fn format_anyhow_chain(err: &anyhow::Error) -> String {
    let mut output = format!("{}", err);
    for cause in err.chain().skip(1) {
        output.push_str(&format!("\nCaused by: {}", cause));
    }

    output
}
