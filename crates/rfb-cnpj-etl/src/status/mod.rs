//! Subcommand entry point, orchestration, and comprehensive unit tests.

pub mod diagnostics;
pub mod models;
pub mod reporter;

pub use diagnostics::*;
pub use models::*;
pub use reporter::*;

use crate::cli::{OutputFormat, StatusArgs};
use anyhow::Result;

/// Executes the `status` subcommand: collects metrics and renders report.
pub fn run(args: StatusArgs) -> Result<StatusReport> {
    let mut args = args;
    if args.json {
        args.format = OutputFormat::Json;
    }
    let report = collect_status_report(&args);
    render_report(&report, args.format)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::TempDir;

    #[test]
    fn test_format_bytes_scaling() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }

    #[test]
    fn test_format_number_commas() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1000000), "1,000,000");
        assert_eq!(format_number(123456789), "123,456,789");
    }

    #[test]
    fn test_styler_formatting() {
        let tty_styler = Styler::with_tty(true);
        assert!(tty_styler.bold("test").contains("\x1b[1m"));
        assert!(tty_styler.green("test").contains("\x1b[1;32m"));
        assert!(tty_styler.red("test").contains("\x1b[1;31m"));
        assert!(tty_styler.yellow("test").contains("\x1b[1;33m"));
        assert!(tty_styler.cyan("test").contains("\x1b[1;36m"));

        let no_tty_styler = Styler::with_tty(false);
        assert_eq!(no_tty_styler.bold("test"), "test");
        assert_eq!(no_tty_styler.green("test"), "test");
        assert_eq!(no_tty_styler.red("test"), "test");
        assert_eq!(no_tty_styler.yellow("test"), "test");
        assert_eq!(no_tty_styler.cyan("test"), "test");
    }

    #[test]
    fn test_status_on_empty_nonexistent_paths() {
        let temp = TempDir::new().unwrap();
        let args = StatusArgs {
            cnpj_db: temp.path().join("nonexistent_cnpj.db"),
            rede_db: temp.path().join("nonexistent_rede.db"),
            search_db: temp.path().join("nonexistent_search.db"),
            zip_dir: temp.path().join("nonexistent_zips"),
            format: crate::cli::OutputFormat::Text,
            json: false,
        };

        let report = collect_status_report(&args);
        assert_eq!(report.overall_status, PipelineOverallStatus::Empty);
        assert!(!report.databases.cnpj.common.exists);
        assert!(!report.databases.rede.common.exists);
        assert!(!report.databases.rede_search.common.exists);
        assert!(!report.zip_archives.exists);

        // Assert JSON formatting succeeds without panic
        assert!(render_report(&report, crate::cli::OutputFormat::Json).is_ok());
        // Assert Text formatting succeeds without panic
        assert!(render_report(&report, crate::cli::OutputFormat::Text).is_ok());
    }

    #[test]
    fn test_status_with_synthetic_cnpj_and_rede_dbs() {
        let temp = TempDir::new().unwrap();
        let cnpj_path = temp.path().join("cnpj.db");
        let rede_path = temp.path().join("rede.db");
        let search_path = temp.path().join("rede_search.db");
        let zip_dir = temp.path().join("dados-publicos-zip");

        // Seed synthetic cnpj.db
        {
            let conn = Connection::open(&cnpj_path).unwrap();
            let _: String = conn
                .query_row("PRAGMA journal_mode = WAL;", [], |r| r.get(0))
                .unwrap();
            conn.execute_batch(
                "CREATE TABLE empresas (cnpj_basico TEXT);
                 INSERT INTO empresas VALUES ('11111111');
                 INSERT INTO empresas VALUES ('22222222');
                 CREATE TABLE _referencia (referencia TEXT, valor TEXT);
                 INSERT INTO _referencia VALUES ('CNPJ', '01/10/2026');
                 INSERT INTO _referencia VALUES ('cnpj_qtde', '2');
                 CREATE INDEX idx_empresas_cnpj_basico ON empresas(cnpj_basico);",
            )
            .unwrap();
        }

        // Seed synthetic rede.db
        {
            let conn = Connection::open(&rede_path).unwrap();
            let _: String = conn
                .query_row("PRAGMA journal_mode = WAL;", [], |r| r.get(0))
                .unwrap();
            conn.execute_batch(
                "CREATE TABLE ligacao (id1 TEXT, id2 TEXT, descricao TEXT, comentario TEXT);
                 INSERT INTO ligacao VALUES ('PJ_1', 'PJ_2', 'filial', 'estabelecimento');
                 INSERT INTO ligacao VALUES ('PF_1', 'PJ_1', 'socio', 'socios');
                 CREATE TABLE entidades (id TEXT, nome TEXT, tipo TEXT);
                 INSERT INTO entidades VALUES ('PJ_1', 'Empresa 1', 'PJ');
                 INSERT INTO entidades VALUES ('PF_1', 'Sócio 1', 'PF');
                 CREATE INDEX idx_ligacao_origem ON ligacao(id1);",
            )
            .unwrap();
        }

        // Seed synthetic rede_search.db
        {
            let conn = Connection::open(&search_path).unwrap();
            let _: String = conn
                .query_row("PRAGMA journal_mode = WAL;", [], |r| r.get(0))
                .unwrap();
            conn.execute_batch(
                "CREATE VIRTUAL TABLE id_search USING fts5(id, descricao);
                 INSERT INTO id_search VALUES ('PJ_1', 'Empresa 1');",
            )
            .unwrap();
        }

        // Seed 1 zip file
        std::fs::create_dir_all(&zip_dir).unwrap();
        std::fs::write(zip_dir.join("Empresas0.zip"), b"dummy").unwrap();

        let args = StatusArgs {
            cnpj_db: cnpj_path,
            rede_db: rede_path,
            search_db: search_path,
            zip_dir,
            format: crate::cli::OutputFormat::Json,
            json: true,
        };

        let report = collect_status_report(&args);
        assert_eq!(report.overall_status, PipelineOverallStatus::Partial);
        assert!(report.databases.cnpj.common.exists);
        assert_eq!(
            report.databases.cnpj.common.journal_mode.as_deref(),
            Some("wal")
        );
        assert_eq!(
            report.databases.cnpj.reference_date.as_deref(),
            Some("01/10/2026")
        );
        assert_eq!(report.databases.cnpj.establishment_count_ref, Some(2));
        assert_eq!(report.databases.cnpj.tables.get("empresas"), Some(&Some(2)));

        assert!(report.databases.rede.common.exists);
        assert_eq!(
            report.databases.rede.edge_breakdown.get("estabelecimento"),
            Some(&1)
        );
        assert_eq!(report.databases.rede.edge_breakdown.get("socios"), Some(&1));
        assert_eq!(report.databases.rede.entity_breakdown.get("PJ"), Some(&1));
        assert_eq!(report.databases.rede.entity_breakdown.get("PF"), Some(&1));

        assert!(report.databases.rede_search.common.exists);
        assert_eq!(
            report.databases.rede_search.tables.get("id_search"),
            Some(&Some(1))
        );

        assert!(report.zip_archives.exists);
        assert_eq!(report.zip_archives.total_found, 1);
        assert_eq!(report.zip_archives.missing_files.len(), 36);
    }

    #[test]
    fn test_status_run_with_json_flag_overrides_format() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();
        let args = StatusArgs {
            cnpj_db: base.join("cnpj.db"),
            rede_db: base.join("rede.db"),
            search_db: base.join("rede_search.db"),
            zip_dir: base.join("zips"),
            format: OutputFormat::Text,
            json: true,
        };

        let res = run(args);
        assert!(res.is_ok());
    }
}
