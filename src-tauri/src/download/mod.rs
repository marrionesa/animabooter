//! OS download feature: official-ISO catalog + streaming downloader.
//!
//! The catalog never hardcodes ISO URLs or hashes: distros delete old point
//! releases (a pinned URL rots within months). Each entry instead carries a
//! resolver chain — an official index (SHA256SUMS / releases.json) plus the
//! ISO name pattern — resolved at download time, hash included.

pub mod downloader;

use serde::Serialize;

/// One selectable OS release template.
#[derive(Debug, Clone, Serialize)]
pub struct OsEntry {
    /// Stable catalog id, e.g. "ubuntu".
    pub id: &'static str,
    /// Display name, e.g. "Ubuntu".
    pub name: &'static str,
    /// Icon emoji shown next to the name.
    pub icon: &'static str,
    /// Human description of the edition (localized client-side by id).
    pub description: &'static str,
    /// Download size of the ISO file itself (for the honest size chip).
    pub size_bytes: u64,
    /// Official homepage (opened from the picker).
    pub website: &'static str,
}

/// `list_os_catalog` payload: id + localized key + resolved display info.
#[derive(Debug, Clone, Serialize)]
pub struct OsCatalogItem {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub size_bytes: u64,
    pub website: &'static str,
}

/// The honest, human catalog. Seven entries, all with official resolver
/// chains verified live before shipping (see CHANGELOG of this feature).
pub const CATALOG: &[OsEntry] = &[
    OsEntry {
        id: "ubuntu",
        name: "Ubuntu",
        icon: "🟠",
        description: "Ubuntu Desktop LTS (amd64)",
        // 24.04.4 desktop amd64, measured 2026-11-03
        size_bytes: 6_513_603_584,
        website: "https://ubuntu.com/download/desktop",
    },
    OsEntry {
        id: "xubuntu",
        name: "Xubuntu",
        icon: "🔵",
        description: "Xubuntu LTS — lightweight Xfce (amd64)",
        size_bytes: 5_093_711_360,
        website: "https://xubuntu.org/download/",
    },
    OsEntry {
        id: "lubuntu",
        name: "Lubuntu",
        icon: "🩵",
        description: "Lubuntu LTS — lightweight LXQt (amd64)",
        size_bytes: 5_291_726_848,
        website: "https://lubuntu.me/downloads/",
    },
    OsEntry {
        id: "ubuntu-mate",
        name: "Ubuntu MATE",
        icon: "🟢",
        description: "Ubuntu MATE LTS — classic desktop (amd64)",
        size_bytes: 6_291_456_000,
        website: "https://ubuntu-mate.org/download/",
    },
    OsEntry {
        id: "debian",
        name: "Debian",
        icon: "🌀",
        description: "Debian stable — netinst (amd64)",
        size_bytes: 933_798_912,
        website: "https://www.debian.org/CD/http-ftp/",
    },
    OsEntry {
        id: "fedora",
        name: "Fedora",
        icon: "🎩",
        description: "Fedora Workstation — live (x86_64)",
        size_bytes: 2_925_092_864,
        website: "https://fedoraproject.org/workstation/download",
    },
    OsEntry {
        id: "arch",
        name: "Arch Linux",
        icon: "🏔️",
        description: "Arch Linux rolling — live (x86_64)",
        size_bytes: 1_448_583_168,
        website: "https://archlinux.org/download/",
    },
];

impl OsEntry {
    pub fn by_id(id: &str) -> Option<&'static OsEntry> {
        CATALOG.iter().find(|e| e.id == id)
    }
}
