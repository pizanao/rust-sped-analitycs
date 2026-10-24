use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use ecd_domain::{RegI355, RegJ150};
use ecd_fiscal::ReceitaExtractor;
use ecd_parser::{process_cta, SpedDocument};
use serde::Deserialize;

#[derive(Deserialize)]
struct ApuracaoInput {
    j150: Vec<RegJ150>,
    i355: Vec<RegI355>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Uso: parity_cli <comando> [argumentos...]");
        std::process::exit(1);
    }

    let cmd = args[1].as_str();
    match cmd {
        "process-cta" => {
            let input = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let result = process_cta(input);
            let out = serde_json::json!({ "result": result });
            println!("{}", out);
        }
        "normalizar-texto" => {
            let input = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let result = ReceitaExtractor::normalizar(input);
            let out = serde_json::json!({ "result": result });
            println!("{}", out);
        }
        "fuzz-ratio" => {
            let s1 = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let s2 = args.get(3).map(|s| s.as_str()).unwrap_or("");
            let score = ReceitaExtractor::partial_ratio(s1, s2);
            let out = serde_json::json!({ "score": score, "score_100": score * 100.0 });
            println!("{}", out);
        }
        "parse-file" => {
            let file_path = PathBuf::from(&args[2]);
            let hash = args.get(3).map(|s| s.as_str()).unwrap_or("HASH_TEST");
            let data_entrega = args
                .get(4)
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);

            match SpedDocument::parse_file(&file_path, hash, data_entrega) {
                Ok(doc) => {
                    let out = serde_json::to_string(&doc).unwrap();
                    println!("{}", out);
                }
                Err(err) => {
                    eprintln!("Erro ao parsear arquivo SPED: {:?}", err);
                    std::process::exit(2);
                }
            }
        }
        "filtra-receitas" => {
            let cnpj = args.get(2).map(|s| s.as_str()).unwrap_or("00000000000000");
            let json_path = args.get(3).expect("caminho json para j150");
            let mut file = File::open(json_path).expect("abrir json de reg_j150");
            let mut content = String::new();
            file.read_to_string(&mut content).unwrap();

            let j150_records: Vec<RegJ150> = serde_json::from_str(&content).unwrap();
            let extractor = ReceitaExtractor::new();
            let apuracao = extractor.filtra_receitas_j150(cnpj, &j150_records, None);
            let out = serde_json::to_string(&apuracao).unwrap();
            println!("{}", out);
        }
        "apurar-consolidado" => {
            let json_path = args.get(2).expect("caminho json para apuracao");
            let mut file = File::open(json_path).expect("abrir json de apuracao");
            let mut content = String::new();
            file.read_to_string(&mut content).unwrap();

            let input: ApuracaoInput = serde_json::from_str(&content).unwrap();
            let extractor = ReceitaExtractor::new();
            let apuracao = extractor.apurar_consolidado(&input.j150, &input.i355);
            let out = serde_json::to_string(&apuracao).unwrap();
            println!("{}", out);
        }
        other => {
            eprintln!("Comando desconhecido: {}", other);
            std::process::exit(1);
        }
    }
}
