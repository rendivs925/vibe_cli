#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Apt,
    Dnf,
    Yum,
    Pacman,
    Zypper,
    Apk,
    Xbps,
    Pkg,
    Brew,
    Yay,
    Paru,
    Aurutils,
    Trizen,
    Pikaur,
    Unknown,
}

impl PackageManager {
    pub fn detect() -> Self {
        if command_exists("apt") {
            return Self::Apt;
        }
        if command_exists("dnf") {
            return Self::Dnf;
        }
        if command_exists("yum") {
            return Self::Yum;
        }
        if command_exists("pacman") {
            return Self::Pacman;
        }
        if command_exists("zypper") {
            return Self::Zypper;
        }
        if command_exists("apk") {
            return Self::Apk;
        }
        if command_exists("xbps-query") {
            return Self::Xbps;
        }
        if command_exists("pkg") {
            return Self::Pkg;
        }
        if command_exists("brew") {
            return Self::Brew;
        }
        if command_exists("yay") {
            return Self::Yay;
        }
        if command_exists("paru") {
            return Self::Paru;
        }
        if command_exists("aurutils") {
            return Self::Aurutils;
        }
        if command_exists("trizen") {
            return Self::Trizen;
        }
        if command_exists("pikaur") {
            return Self::Pikaur;
        }
        Self::Unknown
    }

    pub fn install_command(&self, package: &str) -> String {
        match self {
            Self::Apt => format!("sudo apt install -y {package}"),
            Self::Dnf => format!("sudo dnf install -y {package}"),
            Self::Yum => format!("sudo yum install -y {package}"),
            Self::Pacman => format!("sudo pacman -S --noconfirm {package}"),
            Self::Zypper => format!("sudo zypper install -y {package}"),
            Self::Apk => format!("apk add {package}"),
            Self::Xbps => format!("sudo xbps-install -y {package}"),
            Self::Pkg => format!("sudo pkg install -y {package}"),
            Self::Brew => format!("brew install {package}"),
            Self::Yay => format!("yay -S --noconfirm {package}"),
            Self::Paru => format!("paru -S --noconfirm {package}"),
            Self::Aurutils => format!("aur sync {package}"),
            Self::Trizen => format!("trizen -S --noconfirm {package}"),
            Self::Pikaur => format!("pikaur -S --noconfirm {package}"),
            Self::Unknown => format!("echo 'No package manager detected for {package}' && false"),
        }
    }

    pub fn remove_command(&self, package: &str) -> String {
        match self {
            Self::Apt => format!("sudo apt remove -y {package}"),
            Self::Dnf => format!("sudo dnf remove -y {package}"),
            Self::Yum => format!("sudo yum remove -y {package}"),
            Self::Pacman => format!("sudo pacman -R --noconfirm {package}"),
            Self::Zypper => format!("sudo zypper remove -y {package}"),
            Self::Apk => format!("apk del {package}"),
            Self::Xbps => format!("sudo xbps-remove -y {package}"),
            Self::Pkg => format!("sudo pkg delete -y {package}"),
            Self::Brew => format!("brew uninstall {package}"),
            Self::Yay => format!("yay -R --noconfirm {package}"),
            Self::Paru => format!("paru -R --noconfirm {package}"),
            Self::Aurutils => format!("aur sync -c {package}"),
            Self::Trizen => format!("trizen -R --noconfirm {package}"),
            Self::Pikaur => format!("pikaur -R --noconfirm {package}"),
            Self::Unknown => format!("echo 'No package manager detected for {package}' && false"),
        }
    }

    pub fn search_command(&self, query: &str) -> String {
        match self {
            Self::Apt => format!("apt search {query}"),
            Self::Dnf | Self::Yum => format!("dnf search {query}"),
            Self::Pacman => format!("pacman -Ss {query}"),
            Self::Zypper => format!("zypper search {query}"),
            Self::Apk => format!("apk search {query}"),
            Self::Xbps => format!("xbps-query -Rs {query}"),
            Self::Pkg => format!("pkg search {query}"),
            Self::Brew => format!("brew search {query}"),
            Self::Yay => format!("yay -Ss {query}"),
            Self::Paru => format!("paru -Ss {query}"),
            Self::Aurutils => format!("aur search {query}"),
            Self::Trizen => format!("trizen -Ss {query}"),
            Self::Pikaur => format!("pikaur -Ss {query}"),
            Self::Unknown => format!("echo 'No package manager detected for {query}' && false"),
        }
    }

    pub fn update_command(&self) -> String {
        match self {
            Self::Apt => "sudo apt update".to_string(),
            Self::Dnf => "sudo dnf check-update".to_string(),
            Self::Yum => "sudo yum check-update".to_string(),
            Self::Pacman => "sudo pacman -Sy".to_string(),
            Self::Zypper => "sudo zypper refresh".to_string(),
            Self::Apk => "apk update".to_string(),
            Self::Xbps => "sudo xbps-install -S".to_string(),
            Self::Pkg => "sudo pkg update".to_string(),
            Self::Brew => "brew update".to_string(),
            Self::Yay => "yay -Sy".to_string(),
            Self::Paru => "paru -Sy".to_string(),
            Self::Aurutils => "aur fetch".to_string(),
            Self::Trizen => "trizen -Sy".to_string(),
            Self::Pikaur => "pikaur -Sy".to_string(),
            Self::Unknown => "echo 'No package manager detected' && false".to_string(),
        }
    }

    pub fn upgrade_command(&self) -> String {
        match self {
            Self::Apt => "sudo apt upgrade -y".to_string(),
            Self::Dnf => "sudo dnf upgrade -y".to_string(),
            Self::Yum => "sudo yum update -y".to_string(),
            Self::Pacman => "sudo pacman -Syu --noconfirm".to_string(),
            Self::Zypper => "sudo zypper update -y".to_string(),
            Self::Apk => "apk upgrade".to_string(),
            Self::Xbps => "sudo xbps-install -Su".to_string(),
            Self::Pkg => "sudo pkg upgrade -y".to_string(),
            Self::Brew => "brew upgrade".to_string(),
            Self::Yay => "yay -Syu --noconfirm".to_string(),
            Self::Paru => "paru -Syu --noconfirm".to_string(),
            Self::Aurutils => "aur sync --sysupgrade".to_string(),
            Self::Trizen => "trizen -Syu --noconfirm".to_string(),
            Self::Pikaur => "pikaur -Syu --noconfirm".to_string(),
            Self::Unknown => "echo 'No package manager detected' && false".to_string(),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Apt => "apt",
            Self::Dnf => "dnf",
            Self::Yum => "yum",
            Self::Pacman => "pacman",
            Self::Zypper => "zypper",
            Self::Apk => "apk",
            Self::Xbps => "xbps",
            Self::Pkg => "pkg",
            Self::Brew => "brew",
            Self::Yay => "yay",
            Self::Paru => "paru",
            Self::Aurutils => "aurutils",
            Self::Trizen => "trizen",
            Self::Pikaur => "pikaur",
            Self::Unknown => "unknown",
        }
    }
}

fn command_exists(name: &str) -> bool {
    which::which(name).is_ok()
}
