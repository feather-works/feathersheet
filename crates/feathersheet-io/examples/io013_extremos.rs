//! IO-013: estresse controlado do FSHZ (FSH2/Arrow), Unicode, fórmulas e metadados.
//! cargo run --manifest-path feathersheet/crates/feathersheet-io/Cargo.toml \
//!   --example io013_extremos -j 2
//! IO013_LINHAS=65536 cargo run --manifest-path ... --example io013_extremos
use feathersheet_io::{
    fshz_read::ler_fshz,
    fshz_write::{gerar_fshz2, PastaBruta},
};
use serde_json::json;

fn quantidade() -> usize {
    std::env::var("IO013_LINHAS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(16_384)
        .clamp(300, 250_000)
}

fn bitmap(linhas: usize) -> Vec<u8> {
    let mut mascara = vec![0xff; linhas.div_ceil(8)];
    if linhas % 8 != 0 {
        *mascara.last_mut().expect("linhas > 0") = (1 << (linhas % 8)) - 1;
    }
    // Buraco interior: também precisa sobreviver em cada coluna.
    mascara[12 / 8] &= !(1 << (12 % 8));
    mascara
}

fn entrada(linhas: usize) -> Result<PastaBruta, String> {
    let presentes = bitmap(linhas);
    let numeros: Vec<f64> = (0..linhas)
        .map(|i| i as f64 * 1.25 - 350.0)
        .collect();
    let indices: Vec<i32> = (0..linhas).map(|i| (i % 3) as i32).collect();
    // O slot 1 significa resultado numérico do padrão 0, conforme formula_table.
    let formulas: Vec<i32> = vec![1; linhas];
    let resultados: Vec<f64> = (0..linhas).map(|i| (i as f64) * 2.0).collect();
    let config = json!({
        "a": 0,
        "names": {"Conferencia": {"r0": 0, "c0": 0, "r1": 12, "c1": 3}},
        "sheets": [{
            "n": "Medições ⚡ – São José 🇧🇷",
            "linhas": linhas,
            "w": [[1, 145]],
            "h": [[4, 30]],
            "g": [{"type": "line", "title": "Consumo & potência"}],
            "pr": [{"id": "p1", "descricao": "Dados protegidos"}],
            "pd": {"consulta:teste": {"v": "Resposta persistida ⚡", "at": 42}},
            "bt": [{"rotulo": "Ação", "macro": "atualizar"}],
            "avulsas": [
                {"k": 0, "v": "Árvore, café, 你好, ⚡", "estilo": {"bold": true}},
                {"k": 1, "v": "=SOMA(10;20)", "estilo": null}
            ],
            "estilos_literais": [[2, {"cor": "#123456"}]],
            "estilos_coluna": [[2, {"formato": "#,##0.00"}]],
            "estilos_linha": [[4, {"negrito": true}]],
            "estilo_aba": {"fontFamily": "Noto Sans"},
            "colunas_num": [{"col": 2, "values": numeros, "bitmap": presentes}],
            "colunas_txt": [{
                "col": 3, "indices": indices, "bitmap": bitmap(linhas),
                "dict": ["ação", "energia ⚡", "日本語"]
            }],
            "colunas_formula": [{
                "col": 4, "slots": formulas, "payload": resultados,
                "bitmap": bitmap(linhas), "dict": [],
                "erros": [{"code": "#DIV/0!", "detail": "Teste controlado"}]
            }],
            "padroes_formula": ["\u{1}r0;r-1\u{1}*2"]
        }]
    });
    serde_json::from_value(config).map_err(|e| format!("corpus inválido: {e}"))
}

fn conferir(linhas: usize) -> Result<Vec<u8>, String> {
    let bytes = gerar_fshz2(&entrada(linhas)?)?;
    if !bytes.starts_with(b"FSH2\x1f\x8b") {
        return Err("arquivo não tem assinatura FSH2+GZip".into());
    }
    let lido = ler_fshz(&bytes)?;
    if lido.sheets.len() != 1 || lido.sheets[0].n != "Medições ⚡ – São José 🇧🇷" {
        return Err("identidade Unicode da aba perdida".into());
    }
    let aba = &lido.sheets[0];
    if aba.avulsas.iter().map(|c| c.v.as_str()).collect::<Vec<_>>()
        != ["Árvore, café, 你好, ⚡", "=SOMA(10;20)"]
    {
        return Err("texto/fórmula de célula avulsa alterado".into());
    }
    if aba.colunas_num.len() != 1 || aba.colunas_txt.len() != 1
        || aba.colunas_formula.len() != 1
    {
        return Err("colunas densas não foram preservadas".into());
    }
    let nums = &aba.colunas_num[0];
    let textos = &aba.colunas_txt[0];
    let formulas = &aba.colunas_formula[0];
    if nums.values.len() != linhas || textos.indices.len() != linhas
        || formulas.slots.len() != linhas
        || numeros_diferentes(nums.values[linhas - 1], (linhas - 1) as f64 * 1.25 - 350.0)
        || numeros_diferentes(formulas.payload[linhas - 1], (linhas - 1) as f64 * 2.0)
        || textos.dict != ["ação", "energia ⚡", "日本語"]
        || aba.padroes_formula.len() != 1
        || nums.bitmap[12 / 8] & (1 << (12 % 8)) != 0
        || textos.bitmap[12 / 8] & (1 << (12 % 8)) != 0
        || formulas.bitmap[12 / 8] & (1 << (12 % 8)) != 0
    {
        return Err("valores, bitmaps, padrões ou dicionários alterados".into());
    }
    if aba.estilos_coluna.len() != 1 || aba.estilos_linha.len() != 1
        || aba.estilo_aba.is_none() || aba.estilos_diretos.len() != 1
        || aba.g.is_none() || aba.pr.is_none()
        || aba.bt.is_none() || aba.pd.is_none()
        || lido.names.is_none()
    {
        return Err("estilos ou metadados de planilha perdidos".into());
    }
    let corrompido = &bytes[..bytes.len() / 2];
    if ler_fshz(corrompido).is_ok() {
        return Err("arquivo FSHZ truncado deveria falhar".into());
    }
    Ok(bytes)
}

fn numeros_diferentes(a: f64, b: f64) -> bool {
    a.to_bits() != b.to_bits()
}

fn main() {
    let linhas = quantidade();
    let destino = std::env::args().nth(1)
        .unwrap_or_else(|| "/tmp/io013_grande.fshz".into());
    let resultado = (|| -> Result<usize, String> {
        let bytes = conferir(linhas)?;
        std::fs::write(&destino, &bytes).map_err(|e| e.to_string())?;
        let disco = std::fs::read(&destino).map_err(|e| e.to_string())?;
        if disco != bytes {
            return Err("bytes em disco diferem do buffer original".into());
        }
        ler_fshz(&disco)?;
        Ok(bytes.len())
    })();
    match resultado {
        Ok(bytes) => println!("IO-013 FSHZ OK: {bytes} bytes em {destino} ({linhas} linhas validadas; Unicode e metadados)"),
        Err(e) => {
            eprintln!("IO-013 FSHZ FALHOU: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn io013_fshz_unicode_formula_arrow_metadados_e_arquivo_truncado() {
        assert!(conferir(4_096).unwrap().len() > 256);
    }
    #[test]
    fn io013_fshz_bitmap_final_incompleto() {
        assert_eq!(bitmap(301).last().copied(), Some(0x1f));
        assert_eq!(bitmap(304).last().copied(), Some(0xff));
    }
}
