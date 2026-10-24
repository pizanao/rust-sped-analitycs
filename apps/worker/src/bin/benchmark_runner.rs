use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use std::time::Instant;

use chrono::Utc;
use ecd_fiscal::ReceitaExtractor;
use ecd_parser::SpedDocument;

fn get_rss_kb() -> usize {
    if let Ok(mut file) = File::open("/proc/self/status") {
        let mut content = String::new();
        if file.read_to_string(&mut content).is_ok() {
            for line in content.lines() {
                if line.starts_with("VmRSS:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<usize>() {
                            return kb;
                        }
                    }
                }
            }
        }
    }
    0
}

fn generate_synthetic_sped(path: &Path, target_mb: usize) -> usize {
    let mut f = BufWriter::new(File::create(path).expect("criar arquivo sintético"));

    // Header 0000
    writeln!(
        f,
        "|0000|LECD|01012023|31122023|EMPRESA BENCHMARK S/A|06057223000171|BA|123456789|2927408|654321||0|1|0||0|0||N|N|0|0|1|"
    ).unwrap();
    writeln!(f, "|I001|0|").unwrap();
    writeln!(f, "|I010|G|9.00|").unwrap();
    writeln!(
        f,
        "|I030|TERMO DE ABERTURA|1|DIARIO GERAL|100|EMPRESA BENCHMARK S/A|29300012345|06057223000171|10012015||SALVADOR|31122023|"
    ).unwrap();

    let mut line_count = 4;
    let target_bytes = target_mb * 1024 * 1024;
    let mut current_bytes = 0;

    // Gerar plano de contas
    for i in 1..=500 {
        writeln!(
            f,
            "|I050|01012023|04|A|4|3.1.{:04}|3.1|CONTA DE RECEITA BENCHMARK {:04}|",
            i, i
        )
        .unwrap();
        writeln!(f, "|I051|CC{:02}|3.01.01.01.01.{:02}|", i % 10, i % 50).unwrap();
        writeln!(f, "|I052|CC{:02}|AGL_{:04}|", i % 10, i).unwrap();
        line_count += 3;
    }

    writeln!(f, "|I150|01012023|31122023|").unwrap();
    line_count += 1;

    let sample_line = "|I155|3.1.0001|CC01|0,00|C|1000,00|850000,50|849000,50|C|\r\n";
    let sample_len = sample_line.len();

    while current_bytes < target_bytes {
        f.write_all(sample_line.as_bytes()).unwrap();
        current_bytes += sample_len;
        line_count += 1;
    }

    // Demonstracoes
    writeln!(f, "|I350|31122023|").unwrap();
    writeln!(f, "|I355|3.1.0001|CC01|849000,50|C|").unwrap();
    writeln!(f, "|I990|{}|", line_count).unwrap();
    writeln!(f, "|J001|0|").unwrap();
    writeln!(f, "|J005|01012023|31122023|1|DEMONSTRACOES CONTABEIS|").unwrap();
    writeln!(
        f,
        "|J100|AGL_ATIVO|T|1||A|ATIVO TOTAL|500000,00|D|950000,00|D|NE01|"
    )
    .unwrap();
    writeln!(
        f,
        "|J150|1|AGL_REC_SERV|T|1||RECEITA BRUTA DE VENDAS E SERVICOS||C|849000,50|C|R|NE02|"
    )
    .unwrap();
    writeln!(
        f,
        "|J210|0|AGL_LPA|LUCROS ACUMULADOS|100000,00|C|250000,00|C||"
    )
    .unwrap();
    writeln!(f, "|J215|H01|LUCRO DO EXERCICIO|150000,00|C|").unwrap();
    writeln!(f, "|J990|7|").unwrap();

    f.flush().unwrap();
    line_count += 10;
    line_count
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let target_mb = args
        .get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100);

    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("benchmark_sped_{}mb.txt", target_mb));

    println!("=== RUST SPED ANALYTICS - BENCHMARK RUNNER ===");
    println!("Alvo: {} MB em {:?}", target_mb, temp_file);

    let gen_start = Instant::now();
    let total_lines = generate_synthetic_sped(&temp_file, target_mb);
    let gen_duration = gen_start.elapsed();
    let file_size_bytes = fs::metadata(&temp_file).unwrap().len();
    let actual_mb = file_size_bytes as f64 / (1024.0 * 1024.0);

    println!(
        "Arquivo gerado: {:.2} MB ({} linhas) em {:.2?}",
        actual_mb, total_lines, gen_duration
    );

    let initial_rss = get_rss_kb();
    println!(
        "RSS Inicial: {} KB ({:.2} MB)",
        initial_rss,
        initial_rss as f64 / 1024.0
    );

    // Benchmark de Parsing com Zero-Copy Memory Map
    let parse_start = Instant::now();
    let doc = SpedDocument::parse_file(&temp_file, "BENCHMARK_HASH_1234567890ABCDEF", Utc::now())
        .expect("falha ao parsear");
    let parse_duration = parse_start.elapsed();
    let peak_rss = get_rss_kb();

    let parse_secs = parse_duration.as_secs_f64();
    let mb_per_sec = actual_mb / parse_secs;
    let lines_per_sec = total_lines as f64 / parse_secs;
    let rss_diff_kb = peak_rss.saturating_sub(initial_rss);

    println!("--- RESULTADOS DE PARSING SPED ---");
    println!(
        "Tempo de Processamento: {:.3} s ({:?})",
        parse_secs, parse_duration
    );
    println!("Throughput (Vazão): {:.2} MB/s", mb_per_sec);
    println!("Taxa de Linhas: {:.0} linhas/segundo", lines_per_sec);
    println!(
        "Pico RSS: {} KB ({:.2} MB)",
        peak_rss,
        peak_rss as f64 / 1024.0
    );
    println!(
        "Delta RSS: {} KB ({:.2} MB)",
        rss_diff_kb,
        rss_diff_kb as f64 / 1024.0
    );
    println!("Registros I155 decodificados: {}", doc.reg_i155.len());

    // Benchmark de Fuzzy Matching LCS Bit-Paralelo
    println!("--- RESULTADOS DE FUZZY MATCHING (LCS BIT-PARALELO) ---");
    let lcs_start = Instant::now();
    let lcs_iterations = 200_000;
    for _ in 0..lcs_iterations {
        ReceitaExtractor::partial_ratio(
            "receita bruta de prestacao de servicos",
            "receita bruta prestacao servicos de tecnologia",
        );
    }
    let lcs_duration = lcs_start.elapsed();
    let lcs_evals_per_sec = lcs_iterations as f64 / lcs_duration.as_secs_f64();
    println!(
        "{} avaliações parciais em {:?} -> {:.0} avaliações/segundo ({:.2} µs/operação)",
        lcs_iterations,
        lcs_duration,
        lcs_evals_per_sec,
        (lcs_duration.as_secs_f64() * 1_000_000.0) / lcs_iterations as f64
    );

    // Limpeza
    fs::remove_file(&temp_file).ok();
    println!("Arquivo temporário removido com sucesso.");
}
