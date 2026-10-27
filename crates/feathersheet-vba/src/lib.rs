//! Conversor de macros VBA (Excel) para as macros em JavaScript do FeatherSheet.
//!
//! O caminho: [`xlsm::ler`] tira o projeto VBA do `.xlsm`; o léxico e a sintaxe montam a
//! árvore de cada módulo; o gerador escreve JavaScript sobre a API das macros (`sheet`,
//! `wb`, `app` — `src/macros/guia.ts`). O que não tem equivalente não some: vira
//! comentário `// VBA não convertido` no código e entra na lista de pendências.

pub mod formulario;
pub mod gerar;
pub mod lexico;
pub mod ovba;
pub mod sintaxe;
pub mod xlsm;

pub use ovba::{ModuloVba, TipoDeModulo};
pub use xlsm::ProjetoVba;
pub use gerar::{converter, Conversao};

/// Converte as macros de um `.xlsm` (os bytes do arquivo).
pub fn converter_xlsm(bytes: &[u8], nome_do_arquivo: &str) -> Result<Conversao, String> {
    Ok(gerar::converter(&xlsm::ler(bytes)?, nome_do_arquivo))
}

/// Converte módulos dados como texto (o `.bas`/`.cls` exportado pelo editor do Excel, ou o
/// VBA colado): (nome do módulo, código). `abas` liga o codename ao nome da aba
/// (`("Sheet8", "MAIN")`); módulo com o nome de um codename é tratado como módulo da aba.
pub fn converter_texto(modulos: &[(&str, &str)], abas: &[(&str, &str)]) -> Conversao {
    let mapa: std::collections::HashMap<String, String> = abas.iter().map(|(c, n)| ((*c).to_string(), (*n).to_string())).collect();
    let modulos = modulos
        .iter()
        .map(|(nome, codigo)| {
            let tipo = if mapa.keys().any(|c| c.eq_ignore_ascii_case(nome)) {
                TipoDeModulo::Aba
            } else if ["thisworkbook", "estapasta_de_trabalho"].contains(&nome.to_lowercase().as_str()) {
                TipoDeModulo::Pasta
            } else {
                TipoDeModulo::Padrao
            };
            ModuloVba { nome: (*nome).to_string(), tipo, codigo: codigo.replace("\r\n", "\n") }
        })
        .collect();
    let projeto = ProjetoVba { modulos, abas: mapa, nomes_das_abas: abas.iter().map(|(_, n)| (*n).to_string()).collect(), formularios: Vec::new() };
    gerar::converter(&projeto, "")
}
