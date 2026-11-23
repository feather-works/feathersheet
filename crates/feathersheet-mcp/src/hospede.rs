//! Quem guarda a pasta que as ferramentas mexem: um arquivo aberto pelo
//! próprio servidor ([`Avulso`]) ou o aplicativo aberto na tela (implementado
//! no `feathersheet-gui`). As ferramentas só falam com este trait.
use std::path::{Path, PathBuf};

use feathersheet_core::celulas::refs::Rect;
use feathersheet_core::planilha::workbook::Workbook;
use serde_json::Value;

/// Resposta curta das ações que só existem com o aplicativo aberto.
pub const SO_AO_VIVO: &str = "Esta ferramenta age na tela do aplicativo: abra o FeatherSheet e chame de novo.";

pub trait Hospede {
    fn pasta(&self) -> &Workbook;
    fn pasta_mut(&mut self) -> &mut Workbook;
    /// Índice da aba ativa.
    fn aba_ativa(&self) -> usize;
    fn definir_aba_ativa(&mut self, indice: usize);
    /// Arquivo de onde a pasta veio, se veio de um.
    fn caminho(&self) -> Option<PathBuf>;
    /// A pasta mudou (aparece como "não salvo").
    fn marcar_alterado(&mut self);
    /// Depois de escrever fórmula em muitas células: sem o app, calcula agora (em
    /// paralelo); ao vivo, o app calcula noutra thread sem travar a janela.
    fn calcular_muitas(&mut self) { self.pasta_mut().calcular_em_paralelo(); }
    fn alterado(&self) -> bool;
    fn ao_vivo(&self) -> bool { false }

    fn abrir(&mut self, caminho: &Path) -> Result<String, String>;
    fn salvar(&mut self, caminho: Option<&Path>) -> Result<String, String>;

    /// A seleção da tela (só ao vivo).
    fn selecao(&self) -> Option<Rect> { None }
    fn selecionar(&mut self, _area: Rect) -> Result<(), String> { Err(SO_AO_VIVO.into()) }
    /// Um comando da interface pelo nome (ver `comandos`). Devolve o que a barra de status disse.
    fn comando(&mut self, _nome: &str, _args: &Value) -> Result<String, String> { Err(SO_AO_VIVO.into()) }
    /// (nome, o que faz) dos comandos aceitos.
    fn comandos(&self) -> Vec<(&'static str, &'static str)> { Vec::new() }
    /// Texto do estado visível da tela: aba da faixa, menus e janelas abertos, status.
    fn tela(&self) -> Option<String> { None }
    /// Depois de inserir/excluir linhas ou colunas (o motor já deslocou as
    /// células e consertou as fórmulas): acerta as mesclagens e, no
    /// aplicativo, também tabelas, AutoFiltro e totais.
    fn refluir_estrutura(&mut self, aba: usize, coluna: bool, at: u32, quantas: u32, removendo: bool) -> Result<(), String> {
        feathersheet_io::mesclas::refluir(self.pasta_mut(), aba, coluna, at, quantas, removendo)
    }
    /// Sabe acertar tabelas ao inserir/excluir (só o aplicativo, por enquanto:
    /// essa regra ainda mora na interface).
    fn reflui_tabelas(&self) -> bool { false }
    /// AutoFiltro do intervalo `area` (a regra de tabelas e filtros mora no
    /// aplicativo): `acao` aplicar (critério na `coluna`, absoluta), limpar
    /// (critérios; só da `coluna`, se vier), remover (o AutoFiltro) ou listar.
    fn filtrar(&mut self, _aba: usize, _area: Rect, _acao: &str, _coluna: Option<u32>, _criterio: Option<Value>) -> Result<String, String> {
        Err("O filtro usa as tabelas do aplicativo: abra o FeatherSheet (com o MCP ligado nas Configurações) e chame de novo. Sem o app, sheet_consulta filtra para outra aba.".into())
    }
    /// Tabela dinâmica (a regra de layout, filtros e campos calculados mora
    /// no aplicativo): `acao` listar, criar, editar, atualizar ou excluir.
    fn dinamica(&mut self, _args: &Value) -> Result<String, String> {
        Err("A tabela dinâmica usa o painel do aplicativo: abra o FeatherSheet (com o MCP ligado nas Configurações) e chame de novo. Sem o app, sheet_consulta agrupa e soma para outra aba.".into())
    }
    /// O catálogo de macros mudou: o aplicativo recarrega o editor.
    fn macros_mudaram(&mut self) {}
    /// A pessoa liberou as macros deste arquivo? Ao vivo, com o arquivo ainda em
    /// "Ativar macros?", é `false` (e o aviso volta à tela). Sem o app, não há aviso.
    fn macros_liberadas(&mut self) -> bool { true }
    /// O que a macro pediu ao formulário (`hideComponent`, `showComponent`, `field`):
    /// ao vivo a janela acompanha; devolve se há formulário aberto depois disso.
    fn formulario(&mut self, _fechar: bool, _abrir: Option<(String, serde_json::Value)>, _campos: Vec<(String, String, serde_json::Value)>) -> bool { false }
}

/// Sem o aplicativo: uma pasta na memória do servidor, aberta de um arquivo
/// (ou nova). Gravar é sempre pedido explícito.
pub struct Avulso {
    pub pasta: Workbook,
    pub aba: usize,
    pub caminho: Option<PathBuf>,
    pub alterado: bool,
    /// O `<sheetView>` de cada aba do `.xlsx` aberto (linhas de grade, painel congelado, zeros), pelo id
    /// da aba: volta ao salvar `.xlsx`, mesmo com abas criadas ou apagadas no meio.
    pub vistas: std::collections::HashMap<u32, feathersheet_io::xlsx_read::VistaAba>,
}

impl Default for Avulso {
    fn default() -> Self {
        Self { pasta: Workbook::new(), aba: 0, caminho: None, alterado: false, vistas: Default::default() }
    }
}

impl Hospede for Avulso {
    fn pasta(&self) -> &Workbook { &self.pasta }
    fn pasta_mut(&mut self) -> &mut Workbook { &mut self.pasta }
    fn aba_ativa(&self) -> usize { self.aba.min(self.pasta.len().saturating_sub(1)) }
    fn definir_aba_ativa(&mut self, indice: usize) { self.aba = indice; }
    fn caminho(&self) -> Option<PathBuf> { self.caminho.clone() }
    fn marcar_alterado(&mut self) { self.alterado = true; }
    fn alterado(&self) -> bool { self.alterado }

    fn abrir(&mut self, caminho: &Path) -> Result<String, String> {
        let (pasta, aba) = crate::arquivo::abrir(caminho)?;
        self.vistas = crate::arquivo::vistas(caminho).into_iter().enumerate()
            .filter_map(|(i, v)| Some((pasta.aba(i)?.id, v))).collect();
        self.pasta = pasta;
        self.aba = aba;
        self.caminho = Some(caminho.to_path_buf());
        self.alterado = false;
        Ok(format!("Aberto {} ({} aba(s))", caminho.display(), self.pasta.len()))
    }

    fn salvar(&mut self, caminho: Option<&Path>) -> Result<String, String> {
        let destino = caminho.map(Path::to_path_buf).or_else(|| self.caminho.clone())
            .ok_or("A pasta ainda não tem arquivo: informe `caminho` (.fshz).")?;
        crate::arquivo::conferir_sobrescrita(&destino, self.caminho.as_deref())?;
        let vistas = (0..self.pasta.len()).filter_map(|i| self.pasta.aba(i))
            .map(|a| self.vistas.get(&a.id).copied().unwrap_or_default()).collect();
        crate::arquivo::salvar(&self.pasta, self.aba_ativa(), &destino, vistas)?;
        self.caminho = Some(destino.clone());
        self.alterado = false;
        Ok(format!("Salvo em {}", destino.display()))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_io::xlsx_read::VistaAba;

    #[test]
    fn salvar_xlsx_pelo_mcp_mantem_painel_congelado_e_linhas_de_grade() {
        let pasta = std::env::temp_dir().join(format!("fs-mcp-vistas-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let (origem, destino) = (pasta.join("a.xlsx"), pasta.join("b.xlsx"));
        let mut w = Workbook::new();
        w.criar_aba("Segunda");
        let vista = VistaAba { linhas_grade: false, congelar: (2, 1), zeros: true };
        let bytes = feathersheet_io::xlsx_write::com_vistas(vec![vista, VistaAba::default()], || feathersheet_io::xlsx_write::gerar_xlsx(&w)).unwrap();
        std::fs::write(&origem, bytes).unwrap();
        let mut h = Avulso::default();
        h.abrir(&origem).unwrap();
        // Uma aba nova no começo: a vista continua com a aba dela, não com a posição.
        h.pasta.inserir_aba("Nova", 0);
        h.salvar(Some(&destino)).unwrap();
        let lidas = feathersheet_io::xlsx_read::ler_vistas_arquivo(&destino);
        let _ = std::fs::remove_dir_all(&pasta);
        assert_eq!(lidas, vec![VistaAba::default(), vista, VistaAba::default()]);
    }

    #[test]
    fn nao_grava_por_cima_do_xlsx_com_parte_nao_convertida() {
        use std::io::Write;
        let pasta = std::env::temp_dir().join(format!("fs-mcp-sobrescrita-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let origem = pasta.join("a.xlsx");
        // O .xlsx do próprio gravador, com um nome de valor (`Taxa` = 0,1) que a leitura deixa de fora.
        let bytes = feathersheet_io::xlsx_write::gerar_xlsx(&Workbook::new()).unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut saida = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let nome = f.name().to_owned();
            let mut texto = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut texto).unwrap();
            if nome == "xl/workbook.xml" {
                let t = String::from_utf8(texto).unwrap();
                let t = match t.find("<definedNames>") {
                    Some(_) => t.replacen("<definedNames>", "<definedNames><definedName name=\"Taxa\">0.1</definedName>", 1),
                    None => t.replacen("</sheets>", "</sheets><definedNames><definedName name=\"Taxa\">0.1</definedName></definedNames>", 1),
                };
                texto = t.into_bytes();
            }
            saida.start_file(nome, zip::write::SimpleFileOptions::default()).unwrap();
            saida.write_all(&texto).unwrap();
        }
        std::fs::write(&origem, saida.finish().unwrap().into_inner()).unwrap();
        let mut h = Avulso::default();
        h.abrir(&origem).unwrap();
        let erro = h.salvar(None).unwrap_err();
        // O mesmo arquivo escrito de outro jeito também é recusado.
        let desvio = h.salvar(Some(&pasta.join(".").join("a.xlsx")));
        assert!(desvio.is_err(), "pasta/./a.xlsx é o mesmo a.xlsx");
        let outro = h.salvar(Some(&pasta.join("b.xlsx")));
        let _ = std::fs::remove_dir_all(&pasta);
        assert!(erro.contains("Nome definido com valor ou fórmula"), "{erro}");
        assert!(outro.is_ok(), "{outro:?}");
    }
}
