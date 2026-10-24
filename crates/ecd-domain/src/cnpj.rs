use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Cnpj(String);

impl Cnpj {
    pub fn parse(input: &str) -> Result<Self, &'static str> {
        let digits: String = input.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() != 14 {
            return Err("CNPJ deve conter exatamente 14 dígitos numéricos");
        }
        if !Self::validar_digitos(&digits) {
            return Err("Dígitos verificadores do CNPJ inválidos");
        }
        Ok(Self(digits))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn formatado(&self) -> String {
        format!(
            "{}.{}.{}/{}-{}",
            &self.0[0..2],
            &self.0[2..5],
            &self.0[5..8],
            &self.0[8..12],
            &self.0[12..14]
        )
    }

    fn validar_digitos(cnpj: &str) -> bool {
        let bytes = cnpj.as_bytes();
        let nums: Vec<u32> = bytes.iter().map(|b| (b - b'0') as u32).collect();

        // Elimina sequências iguais
        if nums.iter().all(|&n| n == nums[0]) {
            return false;
        }

        // Validação 1º dígito
        let pesos1 = [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
        let soma1: u32 = nums[0..12].iter().zip(pesos1.iter()).map(|(a, b)| a * b).sum();
        let resto1 = soma1 % 11;
        let dig1 = if resto1 < 2 { 0 } else { 11 - resto1 };
        if nums[12] != dig1 {
            return false;
        }

        // Validação 2º dígito
        let pesos2 = [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
        let soma2: u32 = nums[0..13].iter().zip(pesos2.iter()).map(|(a, b)| a * b).sum();
        let resto2 = soma2 % 11;
        let dig2 = if resto2 < 2 { 0 } else { 11 - resto2 };
        nums[13] == dig2
    }
}

impl fmt::Display for Cnpj {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.formatado())
    }
}
