//! A senha de "Proteger planilha" no formato do `<sheetProtection>` do Excel (SHA-512 com sal e giros), para o
//! `.xlsx` abrir protegido no Excel com a mesma senha. Usada pela janela do app, pelas macros e pelo MCP.
use feathersheet_core::planilha::sheet::protecao::SenhaPlanilha;
use sha2::{Digest, Sha512};

/// Giros do hash: o mesmo do Excel.
const GIROS: u32 = 100_000;
/// Arquivo malicioso não pode travar a interface com bilhões de giros.
const GIROS_MAXIMO: u32 = 10_000_000;

fn hash_excel(senha: &str, sal: &[u8], giros: u32) -> Vec<u8> {
    let utf16: Vec<u8> = senha.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut h = Sha512::new().chain_update(sal).chain_update(&utf16).finalize().to_vec();
    for i in 0..giros {
        h = Sha512::new().chain_update(&h).chain_update(i.to_le_bytes()).finalize().to_vec();
    }
    h
}

/// Senha nova, com sal aleatório.
pub fn criar_senha(senha: &str) -> Result<SenhaPlanilha, String> {
    let mut sal = [0_u8; 16];
    getrandom::getrandom(&mut sal).map_err(|_| feathersheet_core::tr!("O gerador criptográfico não está disponível."))?;
    Ok(SenhaPlanilha {
        algoritmo: "SHA-512".into(),
        hash: crate::base64::codificar(&hash_excel(senha, &sal, GIROS)),
        sal: crate::base64::codificar(&sal),
        giros: GIROS,
    })
}

/// A senha digitada confere com a guardada? Comparação em tempo constante.
pub fn conferir(senha: &str, guardada: &SenhaPlanilha) -> Result<bool, String> {
    if !guardada.algoritmo.eq_ignore_ascii_case("SHA-512") {
        return Err(feathersheet_core::trf!("Senha protegida com {}, que o FeatherSheet ainda não confere.", guardada.algoritmo));
    }
    if guardada.giros > GIROS_MAXIMO { return Err(feathersheet_core::tr!("Proteção com número de giros inválido.").into()); }
    let sal = crate::base64::decodificar(&guardada.sal).ok_or(feathersheet_core::tr!("Sal da proteção inválido."))?;
    let hash = crate::base64::decodificar(&guardada.hash).ok_or(feathersheet_core::tr!("Hash da proteção inválido."))?;
    let obtido = hash_excel(senha, &sal, guardada.giros);
    Ok(obtido.len() == hash.len() && obtido.iter().zip(&hash).fold(0u8, |d, (a, b)| d | (a ^ b)) == 0)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn senha_no_formato_do_excel_confere_e_recusa_errada() {
        let s = criar_senha("Fluxo 2026").unwrap();
        assert!(conferir("Fluxo 2026", &s).unwrap());
        assert!(!conferir("fluxo 2026", &s).unwrap());
        assert_eq!(crate::base64::decodificar(&s.hash).unwrap().len(), 64);
        let mut ruim = s.clone();
        ruim.giros = u32::MAX;
        assert!(conferir("x", &ruim).is_err(), "giros absurdos não travam a tela");
    }
}
