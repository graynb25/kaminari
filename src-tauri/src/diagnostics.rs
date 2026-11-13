// Redacted diagnostics report.
//
// The report is generated in Rust from fixed queries and process constants.
// It never reads media titles, URLs, hosts, file paths, extension ids, storage
// values or settings values, so a support copy can be shared without exposing
// the user's library or the sources they use.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::db::Db;

pub const DIAGNOSTICS_FORMAT: &str = "kaminari-diagnostics";
pub const DIAGNOSTICS_VERSION: u32 = 1;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsReport {
    pub format: &'static str,
    pub version: u32,
    pub generated_at: u64,
    pub app_version: &'static str,
    pub os: OsInfo,
    pub database: DatabaseInfo,
    pub features: FeatureFlags,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsInfo {
    pub name: &'static str,
    pub version: String,
    pub architecture: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub schema_version: i64,
    pub row_counts: BTreeMap<&'static str, i64>,
}

/// Coarse, non-sensitive switches only. This deliberately excludes retention
/// values, download folders, account state and every other setting value.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureFlags {
    pub debug_build: bool,
    pub custom_protocol: bool,
    pub proxy_enabled: bool,
    pub incognito_enabled: bool,
    pub extension_storage_encrypted: bool,
}

pub fn collect(
    database: &Db,
    proxy_enabled: bool,
    incognito_enabled: bool,
) -> Result<DiagnosticsReport, String> {
    let snapshot = database
        .diagnostics_snapshot()
        .map_err(|err| err.to_string())?;
    let row_counts = snapshot.row_counts.into_iter().collect();
    Ok(DiagnosticsReport {
        format: DIAGNOSTICS_FORMAT,
        version: DIAGNOSTICS_VERSION,
        generated_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_secs())
            .unwrap_or_default(),
        app_version: env!("CARGO_PKG_VERSION"),
        os: OsInfo {
            name: std::env::consts::OS,
            version: os_version(),
            architecture: std::env::consts::ARCH,
        },
        database: DatabaseInfo {
            schema_version: snapshot.schema_version,
            row_counts,
        },
        features: FeatureFlags {
            debug_build: cfg!(debug_assertions),
            custom_protocol: cfg!(feature = "custom-protocol"),
            proxy_enabled,
            incognito_enabled,
            extension_storage_encrypted: cfg!(windows),
        },
    })
}

pub fn to_json(report: &DiagnosticsReport) -> Result<String, String> {
    let mut json = serde_json::to_string_pretty(report)
        .map_err(|err| format!("cannot encode report: {err}"))?;
    json.push('\n');
    Ok(json)
}

#[cfg(windows)]
fn os_version() -> String {
    use windows::Win32::System::SystemInformation::{GetVersionExW, OSVERSIONINFOW};

    let mut info = OSVERSIONINFOW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is a valid, correctly sized OSVERSIONINFOW that lives for
    // the duration of the call.
    match unsafe { GetVersionExW(&mut info) } {
        Ok(()) => format!(
            "{}.{}.{}",
            info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber
        ),
        Err(_) => "unknown".to_string(),
    }
}

#[cfg(not(windows))]
fn os_version() -> String {
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anilist::{AiringScheduleEntry, CoverImage, Media, MediaTitle};
    use crate::ext_host::ExtStorage;

    fn private_media() -> Media {
        Media {
            id: 4242,
            title: MediaTitle {
                romaji: Some("SECRET TITLE".to_string()),
                english: Some("SECRET ENGLISH TITLE".to_string()),
                native: None,
            },
            cover_image: Some(CoverImage {
                extra_large: Some("https://secret.example/cover.jpg".to_string()),
                large: None,
                color: None,
            }),
            banner_image: Some("https://secret.example/banner.jpg".to_string()),
            description: Some("C:\\Users\\secret\\SECRET SYNOPSIS".to_string()),
            genres: Some(vec!["SECRET GENRE".to_string()]),
            average_score: Some(80),
            season_year: Some(2024),
            format: Some("TV".to_string()),
            status: Some("FINISHED".to_string()),
            episodes: Some(12),
            next_airing_episode: None,
            recommendations: None,
        }
    }

    fn seeded_database() -> Db {
        let db = Db::new(":memory:").unwrap();
        let media = private_media();
        db.add_to_library(&media, "watching").unwrap();
        db.set_watched(&media, 1, true).unwrap();

        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(
            &package.manifest,
            &package.code,
            crate::ext_host::ExtensionTrust::Builtin,
        )
        .unwrap();
        db.source_library_add(
            &package.manifest.id,
            "secret-show-id",
            "SECRET SOURCE TITLE",
            Some("https://secret.example/source.jpg"),
            Some(2024),
            None,
            None,
        )
        .unwrap();
        db.source_progress_set(
            &package.manifest.id,
            "secret-show-id",
            "secret-episode-id",
            1,
            Some("SECRET EPISODE TITLE"),
            12.0,
            600.0,
            false,
        )
        .unwrap();
        db.set(
            &package.manifest.id,
            "secret-storage-key",
            "SECRET_TOKEN_VALUE",
        )
        .unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_secs() as i64)
            .unwrap_or_default();
        db.airing_schedule_replace(
            &[media.id],
            &[AiringScheduleEntry {
                anilist_id: media.id,
                episode: 1,
                airing_at: now + 3600,
            }],
        )
        .unwrap();
        db
    }

    #[test]
    fn diagnostics_report_contains_versions_and_counts() {
        let db = seeded_database();
        let report = collect(&db, true, false).unwrap();

        assert_eq!(report.format, DIAGNOSTICS_FORMAT);
        assert_eq!(report.version, DIAGNOSTICS_VERSION);
        assert_eq!(report.app_version, env!("CARGO_PKG_VERSION"));
        assert!(!report.os.version.is_empty());
        assert_eq!(report.os.name, std::env::consts::OS);
        assert_eq!(report.os.architecture, std::env::consts::ARCH);
        assert_eq!(report.database.schema_version, 4);
        for table in crate::db::DIAGNOSTICS_TABLES {
            assert_eq!(
                report.database.row_counts.get(table),
                Some(&1),
                "missing or wrong count for {table}"
            );
        }
        assert!(report.features.proxy_enabled);
        assert!(!report.features.incognito_enabled);
        assert_eq!(report.features.extension_storage_encrypted, cfg!(windows));
    }

    #[test]
    fn diagnostics_report_excludes_private_values() {
        let db = seeded_database();
        let report = collect(&db, false, true).unwrap();
        let json = to_json(&report).unwrap();
        let package = crate::ext_host::sample_package().unwrap();

        assert!(json.contains("\"appVersion\""));
        assert!(json.contains("\"schemaVersion\""));
        assert!(json.contains("\"rowCounts\""));
        assert!(json.contains("\"media\""));
        assert!(!json.contains("SECRET"));
        assert!(!json.contains("secret"));
        assert!(!json.contains("https://"));
        assert!(!json.contains("http://"));
        assert!(!json.contains(package.manifest.id.as_str()));
        assert!(!json.contains("secret-show-id"));
        assert!(!json.contains("secret-episode-id"));
        assert!(!json.contains("SECRET_TOKEN_VALUE"));
        assert!(!json.contains("C:\\"));
        assert!(!json.contains("/home/"));
        assert!(!json.contains("Cookie"));
        assert!(!json.contains("Authorization"));
        assert!(!json.contains("token"));
    }

    #[test]
    fn diagnostics_report_marks_feature_flags_without_settings_values() {
        let db = Db::new(":memory:").unwrap();
        let report = collect(&db, true, true).unwrap();

        assert!(report.features.proxy_enabled);
        assert!(report.features.incognito_enabled);
        assert_eq!(report.features.debug_build, cfg!(debug_assertions));
        assert_eq!(
            report.features.custom_protocol,
            cfg!(feature = "custom-protocol")
        );
        assert!(report.database.row_counts.values().all(|count| *count == 0));
    }
}
