use colored::{ColoredString, Colorize};

pub fn foreground(text: &str) -> ColoredString {
    text.truecolor(0xCB, 0xE0, 0xF0)
}

pub fn muted(text: &str) -> ColoredString {
    text.truecolor(0x21, 0x49, 0x69)
}

pub fn accent(text: &str) -> ColoredString {
    text.truecolor(0x47, 0xFF, 0x9C)
}

pub fn success(text: &str) -> ColoredString {
    text.truecolor(0x44, 0xFF, 0xB1)
}

pub fn warning(text: &str) -> ColoredString {
    text.truecolor(0xFF, 0xE0, 0x73)
}

pub fn error(text: &str) -> ColoredString {
    text.truecolor(0xE5, 0x2E, 0x2E)
}

pub fn info(text: &str) -> ColoredString {
    text.truecolor(0x0F, 0xC5, 0xED)
}

pub fn magenta(text: &str) -> ColoredString {
    text.truecolor(0xA2, 0x77, 0xFF)
}

pub fn cyan(text: &str) -> ColoredString {
    text.truecolor(0x24, 0xEA, 0xF7)
}

pub fn prompt(text: &str) -> ColoredString {
    cyan(text).bold()
}

pub fn ansi_prompt(prompt: &str) -> String {
    format!(
        "\x1b[38;2;{};{};{}m{}\x1b[0m",
        0x24, 0xEA, 0xF7, prompt
    )
}
