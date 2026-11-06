//! Output rendering: ANSI-styled terminal dashboards and formatted JSON.

use std::io::{stdout, IsTerminal, Write};

use anyhow::Result;

use super::diagnostics::format_number;
use super::models::*;
use crate::cli::OutputFormat;

/// ANSI terminal color styler with automated TTY detection.
pub struct Styler {
    is_tty: bool,
}

impl Styler {
    pub fn new() -> Self {
        Self {
            is_tty: stdout().is_terminal(),
        }
    }

    pub fn with_tty(is_tty: bool) -> Self {
        Self { is_tty }
    }

    pub fn bold(&self, text: &str) -> String {
        if self.is_tty {
            format!("\x1b[1m{}\x1b[0m", text)
        } else {
            text.to_string()
        }
    }

    pub fn green(&self, text: &str) -> String {
        if self.is_tty {
            format!("\x1b[1;32m{}\x1b[0m", text)
        } else {
            text.to_string()
        }
    }

    pub fn red(&self, text: &str) -> String {
        if self.is_tty {
            format!("\x1b[1;31m{}\x1b[0m", text)
        } else {
            text.to_string()
        }
    }

    pub fn yellow(&self, text: &str) -> String {
        if self.is_tty {
            format!("\x1b[1;33m{}\x1b[0m", text)
        } else {
            text.to_string()
        }
    }

    pub fn cyan(&self, text: &str) -> String {
        if self.is_tty {
            format!("\x1b[1;36m{}\x1b[0m", text)
        } else {
            text.to_string()
        }
    }
}

impl Default for Styler {
    fn default() -> Self {
        Self::new()
    }
}

/// Dispatches rendering based on requested output format.
pub fn render_report(report: &StatusReport, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => render_json(report),
        OutputFormat::Text => render_text(report),
    }
}

/// Renders JSON representation to stdout.
pub fn render_json(report: &StatusReport) -> Result<()> {
    let json_str = serde_json::to_string_pretty(report)?;
    let mut out = stdout().lock();
    writeln!(out, "{}", json_str)?;
    out.flush()?;
    Ok(())
}

/// Renders human-readable aligned dashboard to stdout.
pub fn render_text(report: &StatusReport) -> Result<()> {
    let s = Styler::new();
    let mut out = stdout().lock();

    let status_badge = match report.overall_status {
        PipelineOverallStatus::Complete => s.green("[COMPLETE]"),
        PipelineOverallStatus::Partial => s.yellow("[PARTIAL]"),
        PipelineOverallStatus::Empty => s.yellow("[EMPTY]"),
        PipelineOverallStatus::Degraded => s.red("[DEGRADED / CORRUPT]"),
    };

    writeln!(out, "{}", "=".repeat(80))?;
    writeln!(
        out,
        " {} — RFB CNPJ Open Data Pipeline Status",
        s.bold("rfb-cnpj-etl")
    )?;
    writeln!(
        out,
        " Timestamp: {} | Version: {} | Status: {}",
        report.timestamp, report.version, status_badge
    )?;
    writeln!(out, "{}", "=".repeat(80))?;

    // 1. CNPJ DATABASE
    writeln!(out, "\n{}", s.bold("1. CNPJ DATABASE (cnpj.db)"))?;
    let cnpj = &report.databases.cnpj;
    writeln!(out, "   Path:           {}", cnpj.common.path.display())?;
    if cnpj.common.exists {
        let integ = if cnpj.common.integrity_check == "ok" {
            s.green("[OK]")
        } else {
            s.red(&format!("[{}]", cnpj.common.integrity_check))
        };
        writeln!(
            out,
            "   Status:         {}  Size: {} ({} bytes)",
            s.green("[EXISTS]"),
            cnpj.common.size_formatted,
            format_number(cnpj.common.size_bytes)
        )?;
        writeln!(
            out,
            "   Journal Mode:   {:<8}  Integrity: {}",
            cnpj.common.journal_mode.as_deref().unwrap_or("unknown"),
            integ
        )?;
        if let Some(ref d) = cnpj.reference_date {
            let estab_str = cnpj
                .establishment_count_ref
                .map(format_number)
                .unwrap_or_else(|| "none".to_string());
            writeln!(
                out,
                "   Audit Metadata: Reference: {} (Estabelecimentos: {})",
                s.cyan(d),
                estab_str
            )?;
        }
        let idx_str = if cnpj.indices.all_present {
            s.green(&format!(
                "{}/{} [ALL PRESENT]",
                cnpj.indices.total_found, cnpj.indices.total_expected
            ))
        } else {
            s.yellow(&format!(
                "{}/{} [MISSING: {:?}]",
                cnpj.indices.total_found, cnpj.indices.total_expected, cnpj.indices.missing
            ))
        };
        writeln!(out, "   Indices:        {}", idx_str)?;
        writeln!(out, "   {}", "-".repeat(75))?;
        writeln!(
            out,
            "   {:<35} {:>15}   {:<10}",
            "Table Name", "Row Count", "Status"
        )?;
        writeln!(out, "   {}", "-".repeat(75))?;

        let mut total_rows = 0u64;
        for (tbl, cnt_opt) in &cnpj.tables {
            if let Some(cnt) = cnt_opt {
                total_rows += cnt;
                writeln!(
                    out,
                    "   {:<35} {:>15}   {}",
                    tbl,
                    format_number(*cnt),
                    s.green("[OK]")
                )?;
            } else {
                writeln!(out, "   {:<35} {:>15}   {}", tbl, "-", s.red("[MISSING]"))?;
            }
        }
        writeln!(out, "   {}", "-".repeat(75))?;
        writeln!(
            out,
            "   {:<35} {:>15}",
            s.bold("Total Rows (cnpj.db):"),
            format_number(total_rows)
        )?;
    } else {
        writeln!(
            out,
            "   Status:         {}",
            s.yellow("[NOT FOUND] (Not generated yet)")
        )?;
        writeln!(
            out,
            "   Action:         Run 'make build-cnpj' or 'rfb-cnpj-etl build-cnpj'"
        )?;
    }

    // 2. REDE DATABASE
    writeln!(out, "\n{}", s.bold("2. RELATIONAL NETWORK GRAPH (rede.db)"))?;
    let rede = &report.databases.rede;
    writeln!(out, "   Path:           {}", rede.common.path.display())?;
    if rede.common.exists {
        let integ = if rede.common.integrity_check == "ok" {
            s.green("[OK]")
        } else {
            s.red(&format!("[{}]", rede.common.integrity_check))
        };
        writeln!(
            out,
            "   Status:         {}  Size: {} ({} bytes)",
            s.green("[EXISTS]"),
            rede.common.size_formatted,
            format_number(rede.common.size_bytes)
        )?;
        writeln!(
            out,
            "   Journal Mode:   {:<8}  Integrity: {}",
            rede.common.journal_mode.as_deref().unwrap_or("unknown"),
            integ
        )?;
        let idx_str = if rede.indices.all_present {
            s.green(&format!(
                "{}/{} [ALL PRESENT]",
                rede.indices.total_found, rede.indices.total_expected
            ))
        } else {
            s.yellow(&format!(
                "{}/{} [MISSING: {:?}]",
                rede.indices.total_found, rede.indices.total_expected, rede.indices.missing
            ))
        };
        writeln!(out, "   Indices:        {}", idx_str)?;
        writeln!(out, "   {}", "-".repeat(75))?;
        writeln!(
            out,
            "   {:<35} {:>15}   {:<10}",
            "Table Name", "Row Count", "Status"
        )?;
        writeln!(out, "   {}", "-".repeat(75))?;
        for (tbl, cnt_opt) in &rede.tables {
            if let Some(cnt) = cnt_opt {
                writeln!(
                    out,
                    "   {:<35} {:>15}   {}",
                    tbl,
                    format_number(*cnt),
                    s.green("[OK]")
                )?;
            } else {
                writeln!(out, "   {:<35} {:>15}   {}", tbl, "-", s.red("[MISSING]"))?;
            }
        }
        if !rede.edge_breakdown.is_empty() {
            writeln!(out, "   Edge Breakdown (ligacao.comentario):")?;
            for (cat, count) in &rede.edge_breakdown {
                writeln!(out, "     - {:<28} {:>15}", cat, format_number(*count))?;
            }
        }
        if !rede.entity_breakdown.is_empty() {
            writeln!(out, "   Entity Breakdown (entidades.tipo):")?;
            for (cat, count) in &rede.entity_breakdown {
                writeln!(out, "     - {:<28} {:>15}", cat, format_number(*count))?;
            }
        }
    } else {
        writeln!(
            out,
            "   Status:         {}",
            s.yellow("[NOT FOUND] (Not generated yet)")
        )?;
        writeln!(
            out,
            "   Action:         Run 'make build-rede' or 'rfb-cnpj-etl build-rede'"
        )?;
    }

    // 3. FTS5 SEARCH INDEX
    writeln!(out, "\n{}", s.bold("3. FTS5 SEARCH INDEX (rede_search.db)"))?;
    let search = &report.databases.rede_search;
    writeln!(out, "   Path:           {}", search.common.path.display())?;
    if search.common.exists {
        let integ = if search.common.integrity_check == "ok" {
            s.green("[OK]")
        } else {
            s.red(&format!("[{}]", search.common.integrity_check))
        };
        writeln!(
            out,
            "   Status:         {}  Size: {} ({} bytes)",
            s.green("[EXISTS]"),
            search.common.size_formatted,
            format_number(search.common.size_bytes)
        )?;
        writeln!(
            out,
            "   Journal Mode:   {:<8}  Integrity: {}",
            search.common.journal_mode.as_deref().unwrap_or("unknown"),
            integ
        )?;
        writeln!(out, "   {}", "-".repeat(75))?;
        writeln!(
            out,
            "   {:<35} {:>15}   {:<10}",
            "Virtual Table", "Row Count", "Status"
        )?;
        writeln!(out, "   {}", "-".repeat(75))?;
        for (tbl, cnt_opt) in &search.tables {
            if let Some(cnt) = cnt_opt {
                writeln!(
                    out,
                    "   {:<35} {:>15}   {}",
                    tbl,
                    format_number(*cnt),
                    s.green("[OK]")
                )?;
            } else {
                writeln!(out, "   {:<35} {:>15}   {}", tbl, "-", s.red("[MISSING]"))?;
            }
        }
    } else {
        writeln!(
            out,
            "   Status:         {}",
            s.yellow("[NOT FOUND] (Not generated yet)")
        )?;
        writeln!(
            out,
            "   Action:         Run 'make build-rede' or 'rfb-cnpj-etl build-rede'"
        )?;
    }

    // 4. OFFICIAL ZIP ARCHIVES
    writeln!(
        out,
        "\n{}",
        s.bold("4. OFFICIAL ZIP ARCHIVES (dados-publicos-zip)")
    )?;
    let zips = &report.zip_archives;
    writeln!(out, "   Path:           {}", zips.path.display())?;
    if zips.exists {
        let z_badge = if zips.total_found == zips.total_expected {
            s.green(&format!(
                "[{}/{} ARCHIVES PRESENT]",
                zips.total_found, zips.total_expected
            ))
        } else {
            s.yellow(&format!(
                "[{}/{} ARCHIVES PRESENT]",
                zips.total_found, zips.total_expected
            ))
        };
        writeln!(out, "   Status:         {}", z_badge)?;
        writeln!(
            out,
            "   Total Size:     {} ({} bytes)",
            zips.total_size_formatted,
            format_number(zips.total_size_bytes)
        )?;
        if !zips.missing_files.is_empty() && zips.missing_files.len() <= 5 {
            writeln!(out, "   Missing:        {:?}", zips.missing_files)?;
        }
    } else {
        writeln!(
            out,
            "   Status:         {}",
            s.yellow("[DIRECTORY NOT FOUND]")
        )?;
        writeln!(
            out,
            "   Action:         Run 'make download' or 'rfb-cnpj-etl download'"
        )?;
    }

    writeln!(out, "\n{}", "=".repeat(80))?;
    out.flush()?;
    Ok(())
}
