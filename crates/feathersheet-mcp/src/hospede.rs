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
    /// A macro pediu `sheet.showComponent`: ao vivo abre a janela; devolve se abriu.
    fn mostrar_formulario(&mut self, _nome: &str, _dados: &serde_json::Value) -> bool { false }
}

/// Sem o aplicativo: uma pasta na memória do servidor, aberta de um arquivo
/// (ou nova). Gravar é sempre pedido explícito.
pub struct Avulso {
    pub pasta: Workbook,
    pub aba: usize,
    pub caminho: Option<PathBuf>,
    pub alterado: bool,
}

impl Default for Avulso {
    fn default() -> Self {
        Self { pasta: Workbook::new(), aba: 0, caminho: None, alterado: false }
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
        self.pasta = pasta;
        self.aba = aba;
        self.caminho = Some(caminho.to_path_buf());
        self.alterado = false;
        Ok(format!("Aberto {} ({} aba(s))", caminho.display(), self.pasta.len()))
    }

    fn salvar(&mut self, caminho: Option<&Path>) -> Result<String, String> {
        let destino = caminho.map(Path::to_path_buf).or_else(|| self.caminho.clone())
            .ok_or("A pasta ainda não tem arquivo: informe `caminho` (.fshz).")?;
        crate::arquivo::salvar(&self.pasta, self.aba_ativa(), &destino)?;
        self.caminho = Some(destino.clone());
        self.alterado = false;
        Ok(format!("Salvo em {}", destino.display()))
    }
}
