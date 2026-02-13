use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceManager {
    Systemd,
    SysVInit,
    OpenRC,
    Runit,
    Supervisor,
    Unknown,
}

impl ServiceManager {
    pub fn detect() -> Self {
        if Path::new("/run/systemd/system").exists() {
            return Self::Systemd;
        }
        if Path::new("/etc/init.d/openrc").exists() {
            return Self::OpenRC;
        }
        if Path::new("/etc/runit").exists() {
            return Self::Runit;
        }
        if Path::new("/etc/init.d").exists() {
            return Self::SysVInit;
        }
        if command_exists("supervisord") {
            return Self::Supervisor;
        }
        Self::Unknown
    }

    pub fn start_command(&self, service: &str) -> String {
        match self {
            Self::Systemd => format!("sudo systemctl start {service}"),
            Self::SysVInit => format!("sudo service {service} start"),
            Self::OpenRC => format!("sudo rc-service {service} start"),
            Self::Runit => format!("sudo sv up {service}"),
            Self::Supervisor => format!("sudo supervisorctl start {service}"),
            Self::Unknown => format!("echo 'No service manager detected for {service}' && false"),
        }
    }

    pub fn stop_command(&self, service: &str) -> String {
        match self {
            Self::Systemd => format!("sudo systemctl stop {service}"),
            Self::SysVInit => format!("sudo service {service} stop"),
            Self::OpenRC => format!("sudo rc-service {service} stop"),
            Self::Runit => format!("sudo sv down {service}"),
            Self::Supervisor => format!("sudo supervisorctl stop {service}"),
            Self::Unknown => format!("echo 'No service manager detected for {service}' && false"),
        }
    }

    pub fn restart_command(&self, service: &str) -> String {
        match self {
            Self::Systemd => format!("sudo systemctl restart {service}"),
            Self::SysVInit => format!("sudo service {service} restart"),
            Self::OpenRC => format!("sudo rc-service {service} restart"),
            Self::Runit => format!("sudo sv restart {service}"),
            Self::Supervisor => format!("sudo supervisorctl restart {service}"),
            Self::Unknown => format!("echo 'No service manager detected for {service}' && false"),
        }
    }

    pub fn status_command(&self, service: &str) -> String {
        match self {
            Self::Systemd => format!("systemctl status {service}"),
            Self::SysVInit => format!("service {service} status"),
            Self::OpenRC => format!("rc-service {service} status"),
            Self::Runit => format!("sv status {service}"),
            Self::Supervisor => format!("supervisorctl status {service}"),
            Self::Unknown => format!("echo 'No service manager detected for {service}' && false"),
        }
    }

    pub fn enable_command(&self, service: &str) -> String {
        match self {
            Self::Systemd => format!("sudo systemctl enable {service}"),
            Self::SysVInit => format!("sudo chkconfig {service} on"),
            Self::OpenRC => format!("sudo rc-update add {service} default"),
            Self::Runit => format!("sudo ln -s /etc/sv/{service} /var/service/"),
            Self::Supervisor => {
                format!("echo 'Supervisor services are enabled via config for {service}'")
            }
            Self::Unknown => format!("echo 'No service manager detected for {service}' && false"),
        }
    }

    pub fn disable_command(&self, service: &str) -> String {
        match self {
            Self::Systemd => format!("sudo systemctl disable {service}"),
            Self::SysVInit => format!("sudo chkconfig {service} off"),
            Self::OpenRC => format!("sudo rc-update del {service} default"),
            Self::Runit => format!("sudo rm -f /var/service/{service}"),
            Self::Supervisor => {
                format!("echo 'Supervisor services are disabled via config for {service}'")
            }
            Self::Unknown => format!("echo 'No service manager detected for {service}' && false"),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Systemd => "systemd",
            Self::SysVInit => "sysvinit",
            Self::OpenRC => "openrc",
            Self::Runit => "runit",
            Self::Supervisor => "supervisor",
            Self::Unknown => "unknown",
        }
    }
}

fn command_exists(name: &str) -> bool {
    which::which(name).is_ok()
}
