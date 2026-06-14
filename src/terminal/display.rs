use super::terminal::DisplayMode;

pub fn format_bytes(data: &[u8], mode: DisplayMode) -> String {
    if mode == DisplayMode::Raw {
        return String::from_utf8_lossy(data).to_string();
    }

    let mut output = String::with_capacity(data.len() * 2);
    for &byte in data {
        if byte >= 0x20 && byte < 0x7F {
            output.push(byte as char);
        } else {
            match mode {
                DisplayMode::Ignore => {}
                DisplayMode::Hex => {
                    output.push_str(&format!("\\x{:02x}", byte));
                }
                DisplayMode::Placeholder => {
                    output.push('?');
                }
                DisplayMode::Raw => {
                    output.push(byte as char);
                }
            }
        }
    }
    output
}

pub fn next_display_mode(current: DisplayMode) -> DisplayMode {
    match current {
        DisplayMode::Ignore => DisplayMode::Hex,
        DisplayMode::Hex => DisplayMode::Placeholder,
        DisplayMode::Placeholder => DisplayMode::Raw,
        DisplayMode::Raw => DisplayMode::Ignore,
    }
}

pub fn display_mode_name(mode: DisplayMode) -> &'static str {
    match mode {
        DisplayMode::Ignore => "ignore",
        DisplayMode::Hex => "hex",
        DisplayMode::Placeholder => "placeholder",
        DisplayMode::Raw => "raw",
    }
}
