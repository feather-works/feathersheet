//! Abrir e salvar sem o aplicativo, com os mesmos leitores e gravadores dele:
//! `.fshz`, `.xlsx`, `.csv`/`.tsv` para abrir; `.fshz`, `.xlsx`, `.csv`/`.tsv` para salvar.
use std::path::Path;

use feathersheet_core::planilha::workbook::Workbook;

fn extensao(caminho: &Path) -> String {
    caminho.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

/// A pasta e o índice da aba que estava ativa, já calculada.
pub fn abrir(caminho: &Path) -> Result<(Workbook, usize), String> {
    let (mut pasta, aba) = match extensao(caminho).as_str() {
        "fshz" => {
            let lida = feathersheet_io::fshz_read::ler_fshz_arquivo(caminho)?;
            let aba = lida.a as usize;
            (feathersheet_io::para_workbook::para_workbook(&lida), aba)
        }
        "xlsx" => feathersheet_io::xlsx_read::ler_xlsx_arquivo(caminho)?,
        e @ ("csv" | "tsv") => (feathersheet_io::csv_workbook::importar_csv_tsv_arquivo(caminho, e == "tsv", None)?, 0),
        _ => return Err("Formato não suportado: abra .fshz, .xlsx, .csv ou .tsv.".into()),
    };
    pasta.calcular_em_paralelo();
    Ok((pasta, aba))
}

/// Recusa gravar `.xlsx` por cima do arquivo aberto (`origem`) quando ele tem partes que a leitura deixou de fora
/// (formas, nomes com fórmula, validação sem equivalente…): o Ctrl+S do app pede "Salvar como" pelo mesmo motivo.
pub fn conferir_sobrescrita(destino: &Path, origem: Option<&Path>) -> Result<(), String> {
    if extensao(destino) != "xlsx" || origem != Some(destino) { return Ok(()); }
    let Ok(bytes) = std::fs::read(destino) else { return Ok(()) };
    let perdidos = feathersheet_io::xlsx_read::nao_convertidos::nao_convertidos(bytes).unwrap_or_default();
    if perdidos.is_empty() { return Ok(()); }
    let lista: Vec<String> = perdidos.iter().map(|x| match &x.aba {
        Some(a) => format!("{} × {} (aba {a})", x.quantos, x.o_que),
        None => format!("{} × {}", x.quantos, x.o_que),
    }).collect();
    Err(format!("Gravar por cima de {} apagaria o que o FeatherSheet não converte: {}. Informe outro `caminho` (.xlsx ou .fshz).",
        destino.display(), lista.join("; ")))
}

/// O `<sheetView>` de cada aba de um `.xlsx` (vazio para os outros formatos).
pub fn vistas(caminho: &Path) -> Vec<feathersheet_io::xlsx_read::VistaAba> {
    if extensao(caminho) == "xlsx" { feathersheet_io::xlsx_read::ler_vistas_arquivo(caminho) } else { Vec::new() }
}

/// Grava pela extensão do destino. A gravação do `.fshz` é atômica (`.parcial` + renomear).
/// `vistas`: linhas de grade e painel congelado de cada aba, para o `.xlsx` (como o app grava).
pub fn salvar(pasta: &Workbook, aba: usize, caminho: &Path, vistas: Vec<feathersheet_io::xlsx_read::VistaAba>) -> Result<(), String> {
    match extensao(caminho).as_str() {
        "fshz" => feathersheet_io::da_workbook::pasta_gravar_arquivo(pasta, aba as u32, caminho),
        "xlsx" => {
            let bytes = feathersheet_io::xlsx_write::com_vistas(vistas, || feathersheet_io::xlsx_write::gerar_xlsx(pasta))?;
            // Atômico (`.parcial` + renomear): disco cheio no meio não corta o arquivo que já existia.
            feathersheet_io::arquivo::gravar_atomico(caminho, &bytes).map_err(|e| format!("Não consegui gravar {}: {e}", caminho.display()))
        }
        e @ ("csv" | "tsv") => feathersheet_io::arquivo::gravar_atomico_com(caminho, |destino| {
            feathersheet_io::csv_workbook::exportar_delimitado(pasta, aba, e == "tsv", destino)
        })
        .map_err(|e| format!("Não consegui gravar {}: {e}", caminho.display())),
        _ => Err("Salve como .fshz (padrão), .xlsx, .csv ou .tsv (CSV/TSV gravam só a aba ativa, com os valores).".into()),
    }
}
