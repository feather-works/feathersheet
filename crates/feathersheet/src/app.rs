//! Interface nativa: o motor e os arquivos continuam 100% em Rust.
//! O cromo, as janelas auxiliares, as cores e o menu são do feather-ui.
use crate::{configuracao::Configuracao, grade::Grade, edicao_formula::EditorFormula, campo_fx::CampoFx};
use feather_ui::{cromo, janelas::Janelas, tema::*};
use feathersheet_core::{
    celulas::{refs::{cell_name, col_index, Rect, MAX_COLS, MAX_ROWS},
        estilo_parcial::{Campo, EstiloParcial}, style::NumberFormat},
    planilha::{workbook::Workbook, sheet::edicao::Snap},
};
use crate::compat::{keyboard::{self, Modificadores as _}, window};
use pelicano::{Element, Event, Length, Subscription, Task, Size, WindowId, container, row, scrollable, stack, text, text_input, toggler, Space}; use crate::compat::button;
use std::{cell::RefCell, collections::HashSet, path::{Path, PathBuf}, sync::{Arc, Mutex}, time::{Duration, Instant}};
use feather_ui::seletor_cor::{self, Evento as EventoCor};


mod dados;
mod caixas_ui;
mod entrada_rapida;
mod colar_especial;
mod selecao_multipla;
mod ir_para;
mod classificar;
mod atingir_meta;
mod tabela_dados;
mod cenarios_ui;
mod selecao_regiao;
mod preenchimento_ui;
mod ordenacao_pagina;
mod validacao_ui;
mod validacao_dialogo;
mod serie_ui;
mod interface_editor;
mod visoes_planilha;
mod documento_operacoes;
mod edicao_formatacao;
mod despacho_mensagens;
mod despacho_secundario;
mod despacho_teclado;
#[cfg(test)]
mod testes;
#[cfg(test)]
mod medir_colar_grande;
#[cfg(test)]
mod medir_travadas;
#[cfg(test)]
mod rolagem_zoom;
mod explicacao_erro;
mod duplicados_ui;
mod impressao_ui;
pub use impressao_ui::Evento as EventoImpressao;
pub use duplicados_ui::Evento as EventoDuplicados;
mod dialogos;
mod ajuda;
mod monitor;
mod zoom;
pub use feather_ui::monitor::Evento as EventoMonitor;
use feather_ui::monitor::Nivel;
mod ajuda_macros;
mod ajuda_atalhos;
mod macros;
mod botoes_macro;
/// Aba Desenhar: tinta presa às células (caneta, marca-texto, borracha, laço).
mod desenhar;
/// Objetos soltos sobre a grade (formas; depois ícones e SmartArt).
mod objetos;
// Reexportação pública necessária: Msg é pública e carrega EventoVisual.
pub use macros::EventoVisual;
mod menu_nativo;
mod nomes_ui;
mod dialogo_interno;
pub(crate) mod link_ui;
// Base criptográfica migrada; não habilitar UI antes do bloqueio de escrita.
#[allow(dead_code)]
mod protecao_senha;
mod congelar_ui;
mod formulas_ui;
mod inspecao_ui;
mod mcp_vivo;
mod avaliar_ui;
mod configuracoes_ui;
pub(crate) mod limite_memoria;
mod calculo_visivel;
mod menu_principal;
mod miniatura_modelo;
mod faixa_nativa;
mod faixa_nativa_grupos;
mod faixa_responsiva;
mod status_selecao;
mod barra_status;
mod atualizacao;
mod menu_colagem;
mod assistente_divisao;
mod progresso_abertura;
pub use assistente_divisao::Evento as EventoAssistenteDivisao;
mod menu_edicao;
mod menu_bordas;
mod desenho_borda_ui;
/// Menu suspenso com submenus (contexto da célula e da forma).
mod menu_contexto;
mod menu_orientacao;
mod formatar_celulas;
pub use formatar_celulas::Evento as EventoFormatoCelula;
mod graficos;
mod recuperacao;
mod primeiros_passos;
mod novidades;
mod aviso_xlsx;
mod efeitos_imagem;
mod imagens_importadas;
mod imagem_na_celula;
mod formatar_imagem;
pub(crate) mod tabelas;
pub(crate) mod mesclas;
mod renomear_aba;
mod mover_abas;
mod cores_aba;
mod condicional_ui;
mod formulario_macro;
mod estilos_popup;
mod perfil_coluna_ui;
mod estatisticas_ui;
mod ajuste_dimensoes;
mod dimensao_dialogo;
mod celulas_dialogo;
pub(crate) mod topicos_ui;
mod importar_vba;
mod macro_dialogo;
mod historico_macros;
mod eventos_vba;
mod protecao_ui;
mod comentarios_ui;
mod relampago_ui;
pub use comentarios_ui::Evento as EventoComentario;
mod estilo_novo;
mod visibilidade_eixos;
mod openquery_ui;
pub use openquery_ui::Evento as EventoOpenQuery;
pub use formulas_ui::Seta as SetaRastreio;
pub(crate) mod dinamica_ui;
/// Referência viva entre arquivos abertos (`=[Vendas.fshz]Dados!A1`).
pub(crate) mod vinculos;
use crate::condicional;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auxiliar { Configuracoes, Ajuda, Monitor, Macros, FormatarCelulas, DividirTexto, RemoverDuplicados, Imprimir, OpenQuery, Inspecao, PrimeirosPassos, Novidades, Condicional, Formulario }
impl Auxiliar {
    const TODAS: [Auxiliar; 14] = [Self::Configuracoes, Self::Ajuda, Self::Monitor, Self::Macros, Self::FormatarCelulas,
        Self::DividirTexto, Self::RemoverDuplicados, Self::Imprimir, Self::OpenQuery, Self::Inspecao,
        Self::PrimeirosPassos, Self::Novidades, Self::Condicional, Self::Formulario];
}

/// Opções da faixa Preencher, migradas dos gestos de preenchimento do TypeScript.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirecaoPreenchimento { Baixo, Direita, Cima, Esquerda, AteOFim }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbaFaixa { Inicio, Inserir, Desenhar, Layout, Formulas, Dados, Revisao, Exibir, Macros, Ferramentas, Tabela, EditarForma, Dinamica }

impl AbaFaixa {
    /// Chaves fixas da antiga Ribbon TypeScript (ribbon.ts, ABA_KEY),
    /// sem acentos ou rótulos traduzidos no arquivo de configuração.
    fn chave_persistencia(self) -> Option<&'static str> {
        match self {
            Self::Inicio => Some("inicio"),
            Self::Inserir => Some("inserir"),
            Self::Desenhar => Some("desenhar"),
            Self::Layout => Some("layout"),
            Self::Formulas => Some("formulas"),
            Self::Dados => Some("dados"),
            Self::Revisao => Some("revisao"),
            Self::Exibir => Some("exibir"),
            Self::Macros => Some("macros"),
            Self::Ferramentas => Some("ferramentas"),
            // Abas contextuais não são preferência global.
            Self::Tabela | Self::EditarForma | Self::Dinamica => None,
        }
    }

    fn de_chave_persistencia(chave: &str) -> Self {
        match chave {
            "inserir" => Self::Inserir,
            "desenhar" => Self::Desenhar,
            "layout" => Self::Layout,
            "formulas" => Self::Formulas,
            "dados" => Self::Dados,
            "revisao" => Self::Revisao,
            "exibir" => Self::Exibir,
            "macros" => Self::Macros,
            "ferramentas" => Self::Ferramentas,
            _ => Self::Inicio,
        }
    }
}

/// Tipos da mesma árvore de regras do painel TypeScript original.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoCondicional {
    Maior, MaiorIgual, Menor, MenorIgual, Igual, Diferente, Entre, Fora,
    Vazia, NaoVazia, Contem, NaoContem, Comeca, Termina,
    DataAntes, DataDepois, DataIgual, Duplicado, Unico,
    AcimaMedia, AbaixoMedia, Formula, Escala, Barra,
}
impl TipoCondicional {
    const TODOS: [Self; 24] = [
        Self::Maior,Self::MaiorIgual,Self::Menor,Self::MenorIgual,Self::Igual,Self::Diferente,
        Self::Entre,Self::Fora,Self::Vazia,Self::NaoVazia,Self::Contem,Self::NaoContem,
        Self::Comeca,Self::Termina,Self::DataAntes,Self::DataDepois,Self::DataIgual,
        Self::Duplicado,Self::Unico,Self::AcimaMedia,Self::AbaixoMedia,
        Self::Formula,Self::Escala,Self::Barra,
    ];
    /// Inverso tipado dos operadores do motor TS/Rust ao abrir uma regra existente.
    fn de_operador(op: feathersheet_core::operacoes::rules::CellOp) -> Self {
        use feathersheet_core::operacoes::rules::CellOp as O;
        match op {
            O::GreaterThan => Self::Maior,
            O::GreaterOrEqual => Self::MaiorIgual,
            O::LessThan => Self::Menor,
            O::LessOrEqual => Self::MenorIgual,
            O::Equal => Self::Igual,
            O::NotEqual => Self::Diferente,
            O::Between => Self::Entre,
            O::NotBetween => Self::Fora,
            O::IsEmpty => Self::Vazia,
            O::IsNotEmpty => Self::NaoVazia,
            O::TextContains => Self::Contem,
            O::TextNotContains => Self::NaoContem,
            O::TextStartsWith => Self::Comeca,
            O::TextEndsWith => Self::Termina,
            O::DateBefore => Self::DataAntes,
            O::DateAfter => Self::DataDepois,
            O::DateOn => Self::DataIgual,
            O::Duplicate => Self::Duplicado,
            O::Unique => Self::Unico,
            O::AboveAverage => Self::AcimaMedia,
            O::BelowAverage => Self::AbaixoMedia,
            O::Formula => Self::Formula,
        }
    }

    fn titulo(self) -> &'static str {
        match self {
            Self::Maior=>tr!("Maior que"),Self::MaiorIgual=>tr!("Maior ou igual"),
            Self::Menor=>tr!("Menor que"),Self::MenorIgual=>tr!("Menor ou igual"),
            Self::Igual=>tr!("Igual"),Self::Diferente=>tr!("Diferente"),
            Self::Entre=>tr!("Entre"),Self::Fora=>tr!("Fora do intervalo"),
            Self::Vazia=>tr!("Célula vazia"),Self::NaoVazia=>tr!("Célula não vazia"),
            Self::Contem=>tr!("Texto contém"),Self::NaoContem=>tr!("Texto não contém"),
            Self::Comeca=>tr!("Texto começa"),Self::Termina=>tr!("Texto termina"),
            Self::DataAntes=>tr!("Data anterior"),Self::DataDepois=>tr!("Data posterior"),
            Self::DataIgual=>tr!("Data é"),
            Self::Duplicado=>tr!("Duplicados"),Self::Unico=>tr!("Únicos"),
            Self::AcimaMedia=>tr!("Acima da média"),Self::AbaixoMedia=>tr!("Abaixo da média"),
            Self::Formula=>tr!("Fórmula personalizada"),Self::Escala=>tr!("Escala de cores"),
            Self::Barra=>tr!("Barra de dados"),
        }
    }
    fn operador(self) -> Option<&'static str> {
        Some(match self {
            Self::Maior=>"greaterThan",Self::MaiorIgual=>"greaterOrEqual",
            Self::Menor=>"lessThan",Self::MenorIgual=>"lessOrEqual",
            Self::Igual=>"equal",Self::Diferente=>"notEqual",
            Self::Entre=>"between",Self::Fora=>"notBetween",
            Self::Vazia=>"isEmpty",Self::NaoVazia=>"isNotEmpty",
            Self::Contem=>"textContains",Self::NaoContem=>"textNotContains",
            Self::Comeca=>"textStartsWith",Self::Termina=>"textEndsWith",
            Self::DataAntes=>"dateBefore",Self::DataDepois=>"dateAfter",
            Self::DataIgual=>"dateOn",Self::Duplicado=>"duplicate",
            Self::Unico=>"unique",Self::AcimaMedia=>"aboveAverage",
            Self::AbaixoMedia=>"belowAverage",Self::Formula=>"formula",
            Self::Escala|Self::Barra=>return None,
        })
    }
    fn aridade(self) -> usize {
        match self {
            Self::Vazia|Self::NaoVazia|Self::Duplicado|Self::Unico|
            Self::AcimaMedia|Self::AbaixoMedia|Self::Escala|Self::Barra=>0,
            Self::Entre|Self::Fora=>2,
            _=>1,
        }
    }
}
impl std::fmt::Display for TipoCondicional {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
        f.write_str(self.titulo())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formatacao {
    Negrito, Italico, Porcentagem, Moeda,
    Esquerda, Centro, Direita, Limpar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estrutura { InserirLinha, ExcluirLinha, InserirColuna, ExcluirColuna }

/// Região acionada pelo botão direito; uma seleção pode abranger mais de uma célula.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contexto { Celula(u32, u32), Linha(u32), Coluna(u32), Aba(usize) }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pendente { Novo, Abrir, Recente, RecuperarTemporario, Sair, Modelo(&'static str) }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlvoCor { Texto, Fundo }

/// O motor é carregado em uma thread de trabalho e instalado uma única vez
/// na thread da interface. Arc evita copiar milhões de células ao entregar o evento.
#[derive(Clone)]
pub struct PastaLida {
    caminho: PathBuf,
    dados: Arc<Mutex<Option<(Workbook, usize)>>>,
    /// Tempo de leitura e decodificação, em ms (vai para o console).
    ms: u64,
}

impl std::fmt::Debug for PastaLida {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PastaLida").field("caminho", &self.caminho).finish()
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    CliqueAba(usize), CancelarNomeAba,
    ValidacaoAbrir, ValidacaoFechar, ValidacaoSalvar, ValidacaoRemover,
    ExplicacaoErroAbrir, ExplicacaoErroFechar, ExplicacaoErroIrPara(usize),
    ValidacaoPermitir(&'static str), ValidacaoOperador(String),
    ValidacaoAba(validacao_ui::AbaValidacao),
    ValidacaoPrimeiro(String), ValidacaoSegundo(String),
    ValidacaoMensagem(String), ValidacaoEv(validacao_dialogo::Evento),
    Serie(serie_ui::Evento),
    ValidacaoListaAlternar, ValidacaoListaEscolher(String), ValidacaoListaPagina(i32),
    Grafico(graficos::Evento),
    Dinamica(dinamica_ui::Evento),
    TabelaAbrir, TabelaNome(String), TabelaCabecalho(bool),
    TabelaConfirmar, TabelaCancelar, TabelaAcao(tabelas::Acao),
    /// Arquivo escolhido no diálogo Abrir (a leitura começa depois).
    ArquivoEscolhido(Option<PathBuf>),
    /// Configurações → IA (MCP): ligar o canal e somente leitura.
    McpAtivo(bool), McpSoLeitura(bool),
    /// Referências a outros arquivos: canal entre janelas, faixa e Editar links.
    Vinculos(vinculos::Evento),
    /// Copia um texto da tela (comando de conexão do MCP…).
    CopiarTexto(String),
    /// Personalizar barra de status (botão direito no rodapé).
    BarraStatus(barra_status::Evento),
    AlternarMenuTabelas, AlternarMenuEstilos, FecharMenusEstilos, LogoPadrao,
    TabelaCriarPeloMenu, TabelaEstiloEscolher(&'static str), EstiloCelulaAplicar(&'static str),
    JanelaRedimensionada(window::Id, Size),
    ImagemLaboratorioAbrir,
    /// Seleciona o objeto e inicia gesto de mover/redimensionar na grade.
    ImagemPressionar(String, imagens_importadas::Gesto),
    /// Imagem dentro da célula: inserir, colocar na célula, tirar dela.
    ImagemCelula(imagem_na_celula::Evento),
    /// Painel lateral Formatar imagem.
    FormatarImagem(formatar_imagem::Evento),
    /// Edita os pixels da última imagem incorporada na aba, sem mover o objeto.
    ImagemEfeitosAbrir,
    ImagemLaboratorioCarregada(Result<Option<(Vec<u8>, String)>, String>),
    ImagemLaboratorioAplicar(&'static str),
    ImagemLaboratorioSalvar,
    ImagemLaboratorioDestino(Option<PathBuf>),
    ImagemLaboratorioFechar,
    Novo, Abrir, RecuperarTemporario, DestinoTemporario(Option<PathBuf>),
    Aberto(Result<Option<PastaLida>, String>),
    /// Pasta nova a partir de um modelo da galeria da tela inicial (id do motor).
    NovoDeModelo(&'static str),
    Salvar, SalvarComo, DestinoSalvar(Option<PathBuf>),
    /// Alterna a janela principal entre tela inteira e janela normal (F11).
    TelaInteira,
    /// Arquivo > Renomear…: escolhe o novo nome e renomeia o arquivo no disco.
    RenomearArquivo, DestinoRenomear(Option<PathBuf>),
    MenuSalvarCopia, DestinoCopia(Option<PathBuf>),
    BackupCriar, BackupCriado(Result<PathBuf, String>),
    /// Fim da gravação em segundo plano: destino, se era cópia, e o custo em ms ou o erro.
    Gravado(PathBuf, bool, Result<u64, String>),
    ExportarXlsx, DestinoXlsx(Option<PathBuf>),
    ExportarJson, DestinoJson(Option<PathBuf>),
    NomesAbrir(bool), NomesFechar, NomeAtualizar(String), NomeCriar,
    NomeRemover(String), NomeIrPara(String), NomesEvento(nomes_ui::Evento),
    LinkEditar, LinkFechar, LinkEndereco(String), LinkTexto(String),
    LinkSalvar, LinkRemover, LinkAbrir, LinkAbrirEm(u32, u32),
    BotaoMacroNovo, BotaoMacroEditar(String), BotaoMacroRotulo(String),
    BotaoMacroEscolher(String), BotaoMacroSalvar, BotaoMacroExcluir,
    BotaoMacroCancelar, BotaoMacroExecutar(String),
    BotaoMacroPressionar(String, botoes_macro::Gesto),
    /// Controles da aba Desenhar (canetas, borrachas, laço, formas, parar).
    Desenhar(feather_ui::tinta::Evento),
    /// O que a camada de tinta capturou sobre a grade.
    Tinta(feather_ui::tinta::EventoCamada),
    /// Preferências da caneta (pressão, curva, velocidade, indicador).
    Caneta(desenhar::AjusteCaneta),
    /// Abre (Some) ou fecha (None) um submenu do menu de contexto.
    ContextoSub(Option<u8>),
    /// Ação de um submenu do menu de contexto: fecha o menu e executa.
    ContextoExecutar(Box<Msg>),
    /// Camada de objetos soltos: menu Formas, gestos, barra de formatação.
    Objetos(objetos::Evento),
    CongelarMenu, CongelarFechar, CongelarAplicar(u8),
    ExportarDelimitado(bool), DestinoDelimitado(Option<PathBuf>, bool),
    Selecionar(u32, u32), SelecionarLinha(u32), SelecionarColuna(u32),
    EstenderLinha(u32), EstenderColuna(u32), SelecionarTudo, SelecionarRegiaoAtual, AjustarDivisoria(bool, u32), DimensaoArrastada(bool, u32, u32), Estender(u32, u32), ArrastarReferencia(usize, feathersheet_core::formula::ref_drag::Pegada, (u32,u32), (u32,u32)), Editar(String), Digitar(String), InserirQuebraLinha, EditarMultilinha, Confirmar,
    /// Duplo clique: célula e retângulo em pixels relativos ao Canvas.
    EditarNaCelula(u32, u32, f32, f32, f32, f32),
    EnviarEditor, AceitarSugestao(usize), NavegarSugestao(i32), PosicionarCursorFx(usize), SelecionarTextoFx(usize,usize),
    /// Movimento do mouse em alguma janela (id da janela, posição nela).
    CursorJanela(window::Id, pelicano::Point),
    /// Procurar e instalar atualização (Configurações → Sobre).
    Atualizacao(atualizacao::Evento),
    Rolar(i32,i32), RolarNaGrade(u32,u32,i32,bool), Roda(i32), ControleZoom(bool),
    TeclaLiberada(keyboard::Key), Copiar, Recortar, ColarSolicitar, ColarTextoLido(String), ColarImagemLida(Option<Vec<u8>>), Colar(String),
    ColarValoresSolicitar, ColarValores(String), ColarFormatoSolicitar, ColarFormato(String),
    ColarEspecialSolicitar(ModoColagem), ColarEspecial(String, ModoColagem),
    AlternarMenuColagem, FecharMenuColagem,
    AlternarMenuDivisao,
    AlternarMenuEdicao(&'static str), FecharMenuEdicao, EdicaoMenuAcao(Box<Msg>),
    AlternarMenuBordas, FecharMenuBordas,
    /// Caneta de borda (Bordas ▾ > Desenhar bordas).
    BordaDesenho(desenho_borda_ui::Evento),
    /// Limpar ▾ > Limpar hiperlinks: tira o link da seleção e mantém o texto.
    LimparHiperlinks,
    AlternarMenuOrientacao, FecharMenuOrientacao, OrientacaoEscolher(&'static str),
    FormatarCelulasAbrir, FormatoCelula(EventoFormatoCelula),
    Pincel, AplicarPincel,
    CondicionalAbrir, CondicionalMostrar, CondicionalFechar, CondicionalNovaRegra,
    CondicionalPredefinida(TipoCondicional), CondicionalTipo(TipoCondicional), CondicionalValor(String),
    CondicionalValor2(String), CondicionalIntervalo(String), CondicionalPegar, CondicionalSoSelecao(bool), CondicionalCor(String), CondicionalCorTexto(String),
    CondicionalCorMin(String), CondicionalCorMeio(String), CondicionalCorNegativa(String),
    CondicionalCorAbrir(condicional_ui::CorRegra), CondicionalSeletorCor(feather_ui::seletor_cor::Evento),
    CondicionalPontoMeio(bool), CondicionalOcultarValor(bool),
    CondicionalNegrito(bool), CondicionalItalico(bool),
    CondicionalSublinhado(bool), CondicionalRiscado(bool),
    CondicionalAdicionar, CondicionalEditar(usize), CondicionalRemover(usize), CondicionalMover(usize, i32),
    CondicionalLimparSelecao, CondicionalLimparTudo, CondicionalCoresAlternadas,
    Limpar, Desfazer, Refazer, Mesclar, MesclarModo(mesclas::ModoMescla), InclinarTexto,
    AjustarAltura, Dimensao(dimensao_dialogo::Evento), Celulas(celulas_dialogo::Evento), Topicos(topicos_ui::Evento),
    /// Macros → Importar VBA (do .xlsm aberto ou escolhido) e o arquivo lido.
    Protecao(protecao_ui::Evento),
    /// Comentários das células (aba Revisão, Inserir, botão direito, Limpar).
    Comentario(comentarios_ui::Evento),
    VbaImportar, VbaLido(Result<Option<(String, Vec<u8>)>, String>), NovoEstilo(estilo_novo::Evento),
    OcultarDimensoes(bool), ReexibirDimensoes(bool),
    Preencher(DirecaoPreenchimento),
    /// Alça de preenchimento (canto da seleção) solta: o destino da prévia.
    PreencherAlca(Rect),
    /// Preenchimento relâmpago (Ctrl+E) na coluna da célula ativa.
    PreenchimentoRelampago,
    /// Inserir > Checkbox nas células selecionadas; clique numa caixa.
    CaixasInserir, CaixaAlternar(u32, u32),
    Formatar(Formatacao), Estrutura(Estrutura), LinhasGrade(bool), RealceEixos(bool),
    EstiloMenu(String, Campo), AlternarEstilo(String), InserirMenu(Estrutura, bool),
    InserirFuncao(String), InserirSimbolo(String), MostrarCatalogoFuncoes, Formulas(formulas_ui::Evento), Inspecao(inspecao_ui::Evento),
    /// Pedido do MCP ao vivo e a foto que ele pediu.
    Avaliar(avaliar_ui::Evento),
    Mcp(mcp_vivo::PedidoMcp), McpImagem(mcp_vivo::PedidoMcp, Foto),
    FaixaOpcao(&'static str, &'static str), BuscaComandos(String), BuscaComando(usize),
    AlternarMenuExtras, FecharMenuExtras, AlternarGrupoFaixa(usize), FecharGrupoFaixa,
    TamanhoFonteDelta(i32), Decimais(i8), LimparFormato, LimparTudo, AlterarRecuo(i32), AjustarLargura,
    AlternarBarraFx, AlternarMostrarFormulas, MostrarSobre,
    AbrirSeletorCor(AlvoCor), SeletorCor(EventoCor),
    Mover(i32, i32), LinhaPagina(i32), ColunaPagina(i32),
    Aba(usize), AdicionarAba, DuplicarAba, MoverAba(usize, usize), OcultarAba(usize), ReexibirAba(usize),
    MostrarAbasOcultas, FecharAbasOcultas, AbaFaixa(AbaFaixa),
    NomearAba(String), EditarNomeAba, RenomearAba, RemoverAba, IrPara(String), AlterarEndereco(String),
    Menu, MenuGrupo(usize), MenuEscolher(usize), MenuVoltar, MenuMover(i32),
    MenuPagina(menu_principal::Pagina), MenuAbrirRecente(usize),
    /// Setas do carrossel de modelos (passo -1 ou 1) e alternância para a grade completa.
    CarrosselModelos(i32), VerTodosModelos, BuscarRecentes(String),
    MenuFixarRecente(usize), MenuEsquecerRecente(usize), FecharAplicativo,
    AbrirContexto(Contexto, f32, f32),
    AbrirContextoNaGrade(Contexto, crate::geometria_overlay::PosicaoPopup),
    FecharContexto, ContextoEscolher(usize),
    PonteiroSobreAbas(f32), AlternarPaletaAba, VoltarMenuAba, PintarAba(Option<&'static str>),
    Configuracoes, Ajuda, Monitor, AuxAberta, AuxFechada(window::Id),
    AjudaSecao(usize),
    /// Abre a Ajuda F1 numa seção pelo id ("macros", "atalhos"…).
    AjudaAbrirSecao(&'static str),
    AjudaEvento(feather_ui::ajuda::Evento),
    /// Ajuda ▸ Primeiros passos e o assistente da primeira abertura.
    PrimeirosPassosAbrir,
    PrimeirosPassos(primeiros_passos::Evento),
    /// Ajuda ▸ O que há de novo (abre sozinha depois de atualizar).
    NovidadesAbrir,
    /// Fecha a faixa "sem salvamento automático" acima da grade.
    FaixaSalvamentoFechar,
    /// Fecha o aviso de funções que o Excel não tem.
    AvisoXlsxFechar,
    AvisoMacrosFechar,
    AtivarMacros,
    Novidades(novidades::Evento),
    MonitorEvento(EventoMonitor),
    Fechar(window::Id), FecharAux(Auxiliar), Minimizar(window::Id),
    Maximizar(window::Id), Arrastar(window::Id),
    Teclado(window::Id, keyboard::Key, keyboard::Modifiers),
    /// Atalho de macros consumido pelo editor de texto Iced. Nunca alcança a grade.
    MacroTeclado(window::Id, keyboard::Key, keyboard::Modifiers),
    TecladoNoCampo(window::Id, keyboard::Key, keyboard::Modifiers),
    Tema(bool), TemaAuto, Acento(usize), BarraPropria(bool), Faixa, SecaoConfig(usize),
    Idioma(&'static str), CsvDetectarSeparador(bool),
    AutoSalvar(bool), IntervaloAutoSalvar(u16), LimiteMemoriaGb(u16), Nucleos(u16), CalcularPulso, FonteGrade(&'static str), CorpoGrade(u16),
    CapturarAtalho(&'static str), RestaurarAtalhos,
    FecharMenu, Cancelar, Descartar, SalvarAntes, GuardarTemporariamente,
    /// Resposta à pergunta Sim/Não da macro; fecha a mensagem da macro.
    MacroResponder(bool), MacroMensagemOk,
    Busca, BuscaTexto(String), Substituicao(String), BuscaMaiusculas(bool),
    BuscaInteira(bool), BuscaFormulas(bool), BuscaPasta(bool), Localizar, LocalizarAnterior,
    LocalizarTodos, BuscaIrPara(usize), SubstituirAtual, SubstituirTudo, FecharBusca,
    /// Abre a janela de busca na aba Localizar (`false`) ou Substituir (`true`).
    BuscaAba(bool), BuscaMaisOpcoes, BuscaPorColunas(bool),
    BuscaArrastoInicio, BuscaArrastoMover(pelicano::Point), BuscaArrastoFim,
    /// Barra de rolagem: origem absoluta da linha e/ou da coluna.
    RolarPara(Option<u32>, Option<u32>),
    /// Alt+↓: escolher da lista os valores já digitados na coluna.
    ListaDaColuna,
    ColarEspecialJanela(colar_especial::Evento),
    FormularioMacro(formulario_macro::Evento),
    /// Ctrl+clique numa célula: acrescenta uma área à seleção.
    SelecionarAdicional(u32, u32),
    IrParaJanela(ir_para::Evento),
    Classificar(classificar::Evento),
    AtingirMeta(atingir_meta::Evento),
    TabelaDados(tabela_dados::Evento),
    Cenarios(cenarios_ui::Evento),
    Ordenar(bool), OrdenarPagina(bool), CabecalhoDados(bool), RemoverDuplicados, Duplicados(EventoDuplicados), Impressao(EventoImpressao), ApararEspacos, AutoSoma, AutoConta(&'static str),
    DividirTextoOutro(String), DividirTextoAplicar,
    AbrirAssistenteDivisao, AssistenteDivisao(EventoAssistenteDivisao),
    OpenQuery(EventoOpenQuery),
    PerfilColunaAbrir, PerfilColunaFechar, PerfilColunaIrPara(u32),
    PerfilColunaPronto(u64, Result<perfil_coluna_ui::PerfilVisao, String>),
    EstatisticasAbrir, EstatisticasFechar,
    EstatisticasProntas(u64, Result<estatisticas_ui::Visao, String>),
    Zoom(u16), ZoomMinimo(u16), ZoomMaximo(u16),
    Macros, MacroVisual(EventoVisual), MacroEditar, MacroExecutar,
    MacroAbrir, MacroLido(Result<Option<String>,String>), MacroSalvar, MacroDestino(Option<PathBuf>),
    MacroNome(String), MacroNovoModulo, MacroComponentes, MacroNovoFormulario, MacroGuardarModulo, MacroAbrirModulo(String), MacroExcluirModulo,
    MacroImportarCatalogo, MacroCatalogoLido(Result<Option<String>,String>),
    MacroExportarCatalogo, MacroDestinoCatalogo(Option<PathBuf>),
    MonitorPulso, ResumoPulso,
    /// Redesenha a borda andando do Ctrl+C (só enquanto há área copiada).
    CopiaPulso,
    /// Modo de captura (`FEATHERSHEET_CAPTURAR=arquivo.png`): prepara e fotografa.
    CapturarTela, TelaCapturada(Foto),
    /// Redesenha a barra de abertura do rodapé enquanto o arquivo carrega.
    ProgressoPulso,
    Recuperacao(recuperacao::Evento),
    RecuperacaoAutomatica(bool),
    /// Várias mensagens de um evento só da janela (o pelicano entrega uma por chamada).
    Lote(Vec<Msg>),
}

/// A foto de uma janela: PNG e tamanho em pixels (o `window::Screenshot` do Iced).
#[derive(Debug, Clone)]
pub struct Foto {
    pub png: Arc<Vec<u8>>,
    pub largura: u32,
    pub altura: u32,
}

/// Cópia interna com texto (clipboard do sistema) e estilos (motor Rust).
/// O texto continua compatível com outras aplicações; os estilos são usados
/// somente quando o conteúdo recebido do clipboard coincide com a cópia.
#[derive(Debug, Clone)]
struct AreaCopiada {
    texto: String,
    celulas: Vec<Vec<Snap>>,
    // Valores de fórmulas congelados ao copiar, sem estilos nem referências.
    valores: Vec<Vec<String>>,
    /// Largura de cada coluna copiada (`None` = padrão), para
    /// "Colar especial → Larguras das colunas".
    larguras: Vec<Option<u32>>,
    /// Canto de cima à esquerda da origem: colar desloca as referências
    /// relativas pela distância até ele, como no Excel.
    origem: (u32, u32),
    /// Veio do Ctrl+X: colar leva a fórmula sem deslocar (é mover).
    recortada: bool,
}

/// Formatação capturada separadamente do clipboard e do conteúdo.
#[derive(Debug, Clone)]
struct PincelFormatacao {
    estilos: Vec<Vec<Option<EstiloParcial>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoColagem {
    /// Fórmulas, valores e estilos; mesma operação do Ctrl+V.
    Completa,
    SomenteValores,
    ValoresComFormato,
    FormulasSemFormato,
    Transposta,
    SomenteFormato,
}

/// Planejamento da colagem antes de qualquer escrita na planilha.
/// Uma célula ativa recebe o bloco inteiro; uma área selecionada maior
/// deve ser múltipla exata do bloco para permitir preenchimento em mosaico.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlanoColagem {
    r0: u32,
    c0: u32,
    altura: usize,
    largura: usize,
}

impl PlanoColagem {
    fn calcular(area: &Rect, origem_altura: usize, origem_largura: usize) -> Result<Self, &'static str> {
        if origem_altura == 0 || origem_largura == 0 {
            return Err(tr!("Não há células para colar"));
        }
        let area = area.normalized();
        let selecao_altura = (area.r1 - area.r0) as usize + 1;
        let selecao_largura = (area.c1 - area.c0) as usize + 1;
        let (altura, largura) = if selecao_altura == 1 && selecao_largura == 1 {
            (origem_altura, origem_largura)
        } else if selecao_altura % origem_altura == 0 && selecao_largura % origem_largura == 0 {
            (selecao_altura, selecao_largura)
        } else {
            return Err(tr!("A área de destino deve ter dimensões múltiplas do bloco copiado (ou selecionar uma única célula)"));
        };
        if area.r0 as usize + altura > MAX_ROWS as usize
            || area.c0 as usize + largura > MAX_COLS as usize
        {
            return Err(tr!("A colagem ultrapassa o limite de linhas ou colunas da planilha"));
        }
        Ok(Self { r0: area.r0, c0: area.c0, altura, largura })
    }

    fn area(&self) -> Rect {
        Rect::new(
            self.r0, self.c0,
            self.r0 + self.altura as u32 - 1,
            self.c0 + self.largura as u32 - 1,
        )
    }
}

/// Editor nativo sobre a célula; a posição é capturada pelo Canvas, que
/// já considera scroll, zoom, linhas ocultas e painéis congelados.
#[derive(Debug, Clone, Copy)]
struct EdicaoInline {
    folha: usize,
    linha: u32,
    coluna: u32,
    x: f32,
    y: f32,
    largura: f32,
    altura: f32,
}

pub struct App {
    graficos: graficos::Estado,
    dinamica: Option<dinamica_ui::Estado>,
    /// Opções e área da tabela ainda não criada.
    tabela_area: Option<Rect>,
    tabela_nome: String,
    tabela_cabecalho: bool,
    tabela_branca: bool,
    tabela_editando: Option<String>,
    tabela_nome_novo: String,
    tabela_renomeando: bool,
    tabela_redimensionando: bool,
    tabela_intervalo_novo: String,
    tabela_operador: &'static str,
    tabela_criterio: String,
    tabela_col_filtro: u32,
    tabela_filtro_lista: Vec<String>,
    tabela_filtro_vistos: HashSet<String>,
    tabela_filtro_criterio_valores: HashSet<String>,
    tabela_filtro_indices: Vec<usize>,
    tabela_filtro_marcados: HashSet<String>,
    tabela_filtro_truncado: bool,
    tabela_filtro_busca: String,
    tabela_filtro_inicio_visivel: usize,
    tabela_filtro_proxima_linha: u32,
    tabela_filtro_ultima_linha: u32,
    tabela_filtro_automatico: bool,
    tabela_filtro_geracao: u64,
    tabela_filtro_original: Option<serde_json::Value>,
    tabela_filtro_editado: bool,
    /// Menu suspenso de filtro ancorado na seta do cabeçalho; não é painel lateral.
    tabela_popup_filtro: bool,
    /// Lista de cálculos da seta da célula da linha de totais.
    tabela_popup_totais: bool,
    /// Menu "Personalizar barra de status" aberto.
    barra_status_menu: bool,
    /// Última frase do MCP ao vivo (Configurações → IA (MCP)).
    mcp_estado: String,
    tabela_popup_x: f32,
    /// Y do clique no cabeçalho, relativo ao viewport da grade.
    tabela_popup_y: f32,
    /// 0 valores, 1 operadores, 2 critério; 3 a 9 = `tabelas::filtro_extra::PAG_*`.
    tabela_filtro_pagina: u8,
    /// Cores/tipo da coluna e formulários dos submenus por cor, número e data.
    tabela_filtro_extra: tabelas::filtro_extra::Estado,
    /// Área cliente em px lógicos para posicionar popovers sem recorte.
    tamanho_principal: Size,
    efeitos_imagem: efeitos_imagem::Estado,
    imagens_importadas: imagens_importadas::Cache,
    imagem_selecionada: Option<(u32, String)>,
    /// Menu do botão direito da imagem solta aberto, na posição da janela.
    imagem_contexto: Option<(f32, f32)>,
    formatar_imagem: formatar_imagem::Estado,
    /// Conta de tabela dinâmica grande em segundo plano.
    dinamica_fundo: dinamica_ui::fundo::Fundo,
    arraste_imagem: Option<imagens_importadas::Arraste>,
    condicional: RefCell<condicional::Estado>,
    perfil_coluna: Option<perfil_coluna_ui::PerfilVisao>,
    estatisticas: Option<estatisticas_ui::Estado>,
    estatisticas_geracao: u64,
    openquery: Option<openquery_ui::Estado>,
    nomes_modal: Option<nomes_ui::Estado>,
    link_modal: Option<link_ui::Estado>,
    dimensao_modal: Option<dimensao_dialogo::Estado>,
    celulas_modal: Option<celulas_dialogo::Estado>,
    subtotal: Option<topicos_ui::EstadoSubtotal>,
    /// VBA do .xlsm aberto, convertido e esperando a importação.
    vba_pendente: Option<importar_vba::VbaPendente>,
    /// Pergunta Sim/Não de uma macro esperando a pessoa.
    macro_pergunta: Option<macro_dialogo::PerguntaMacro>,
    /// O formulário aberto por `sheet.showComponent`.
    formulario_macro: Option<formulario_macro::FormularioAberto>,
    /// O `valores` da próxima execução (o botão do formulário).
    macro_valores: Option<serde_json::Map<String, serde_json::Value>>,
    /// As últimas execuções de macro (só na memória), vistas no Alt+F11.
    historico_macros: historico_macros::Historico,
    /// Mensagens de `alert`/`MsgBox` ainda não lidas, uma por vez.
    macro_mensagens: Vec<String>,
    /// Eventos do VBA instalados que rodam sozinhos (aoMudar, aoAtivar).
    eventos_vba: Vec<eventos_vba::EventoVba>,
    /// Macros do arquivo aberto (eventos e botões) esperando a pessoa ativar.
    macros_bloqueadas: bool,
    /// O diálogo "Ativar macros?" está aberto.
    aviso_macros: bool,
    /// Uma macro rodou nesta mensagem: o que ela escreveu não dispara evento.
    macro_rodou: bool,
    /// Profundidade de `update` (só a mais de fora dispara eventos).
    profundidade_update: u32,
    protecao_modal: Option<protecao_ui::Estado>,
    /// Editor de comentário aberto (Novo/Editar comentário).
    comentario_editor: Option<comentarios_ui::Estado>,
    /// Célula comentada sob o mouse (ou escolhida por Anterior/Próximo): balão à mostra.
    comentario_sobre: Option<(u32, u32)>,
    /// Revisão > Mostrar todos os comentários (só a tela).
    comentarios_todos: bool,
    novo_estilo: Option<estilo_novo::Estado>,
    /// Janela "Colar especial…" aberta, e as escolhas da última colagem.
    colar_especial: Option<colar_especial::Estado>,
    /// Áreas anteriores da seleção múltipla (Ctrl+clique); a ativa não entra.
    selecoes_extras: Vec<Rect>,
    /// Janela "Ir para" (F5) com as opções do Ir para especial.
    ir_para: Option<ir_para::Estado>,
    /// Janela "Classificar" (vários níveis).
    classificar: Option<classificar::Estado>,
    /// Janela "Atingir meta" (não modal: cliques na grade preenchem campos).
    atingir_meta: Option<atingir_meta::Estado>,
    /// Janela "Tabela de dados" e a última tabela feita (para Atualizar).
    tabela_dados: Option<tabela_dados::Estado>,
    tabela_dados_ultima: Option<tabela_dados::Ultima>,
    /// Gerenciador de cenários (não modal).
    cenarios: Option<cenarios_ui::Estado>,
    colar_especial_ultimo: colar_especial::Estado,
    congelados: std::collections::HashMap<u32, (u32, u32)>,
    /// Abas (id) cujo arquivo do Excel desligava as linhas de grade
    /// (`showGridLines="0"`); valem junto com a opção global.
    grade_oculta: std::collections::HashSet<u32>,
    congelar_menu_aberto: bool,
    /// Aba Desenhar: ferramenta, canetas, seleção do laço e apagar em andamento.
    tinta: desenhar::EstadoTinta,
    /// Formas sobre a grade: seleção, forma a inserir, menu e painel.
    objetos: objetos::EstadoObjetos,
    congelar_popup: (f32, f32),
    perfil_geracao: u64,
    condicional_aberto: bool,
    /// "Aplica-se a" da regra: um ou mais endereços separados por `;`.
    condicional_intervalo: String,
    /// Selecionar na grade preenche o "Aplica-se a" enquanto ligado.
    condicional_pegando: bool,
    /// A lista mostra só as regras que tocam a seleção atual.
    condicional_so_selecao: bool,
    /// Campo de cor da regra com o seletor aberto, e o estado do seletor.
    condicional_cor_aberta: Option<condicional_ui::CorRegra>,
    condicional_hsva: [f32; 4],
    condicional_hex: String,
    condicional_menu_aberto: bool,
    condicional_menu_x: f32,
    /// Âncora vertical do popup, calculada ao clicar na ferramenta da Ribbon.
    condicional_menu_y: f32,
    tabelas_menu_aberto: bool,
    estilos_menu_aberto: bool,
    estilos_popup_x: f32,
    estilos_popup_y: f32,
    tabela_estilo_pendente: Option<&'static str>,
    ponteiro_x: f32,
    ponteiro_y: f32,
    condicional_editando: Option<usize>,
    condicional_operador: TipoCondicional,
    condicional_valor: String,
    condicional_valor2: String,
    condicional_cor: String,
    condicional_cor_texto: String,
    condicional_cor_min: String,
    condicional_cor_meio: String,
    condicional_cor_negativa: String,
    condicional_ponto_meio: bool,
    condicional_ocultar_valor: bool,
    condicional_negrito: bool,
    condicional_italico: bool,
    condicional_sublinhado: bool,
    condicional_riscado: bool,
    workbook: Workbook,
    eixos_grade: RefCell<crate::grade::EstadoEixos>,
    duplicados: Option<duplicados_ui::Estado>,
    /// Área de impressão, quebras e diálogo: só em memória, por id de aba.
    impressao: impressao_ui::Dados,
    resumo_selecao: RefCell<status_selecao::Estado>,
    /// Desenho das células em cache (ver `Grade::draw` e `update`).
    cache_grade: crate::grade::CacheGrade,
    /// Procurar atualizações (Configurações → Sobre), só a pedido.
    atualizacao: atualizacao::Estado,
    config: Configuracao,
    janelas: Janelas<Auxiliar>,
    ultimo_clique_barra: Option<Instant>,
    caminho: Option<PathBuf>,
    /// Cópia de recuperação automática e as cópias de sessões que morreram.
    recuperacao: recuperacao::Estado,
    /// Arquivo de outro formato aberto (`.xlsx`, `.csv`…): ainda sem
    /// `.fshz`, mas é o nome que o título e os diálogos de salvar sugerem.
    origem: Option<PathBuf>,
    alterado: bool,
    /// Estado de tela inteira da janela principal (F11 / Msg::TelaInteira).
    tela_inteira: bool,
    folha: usize,
    linha: u32,
    coluna: u32,
    fim_selecao: (u32, u32),
    linha_inicio: u32,
    coluna_inicio: u32,
    // Mantém a última linha no rodapé quando o zoom altera quantas cabem.
    ancorar_rodape: bool,
    edicao: String,
    edicao_inline: Option<EdicaoInline>,
    /// Buffer editável com quebras de linha dentro da célula.
    multilinha: Option<pelicano::Content>,
    editor_formula: EditorFormula,
    endereco: String,
    /// true enquanto o usuário digita na caixa de nome; falso, a caixa mostra o nome
    /// da tabela quando a seleção é exatamente a área dela.
    endereco_digitado: bool,
    editando_nome_aba: bool,
    aba_em_renomeacao: Option<u32>,
    erro_nome_aba: Option<String>,
    ultimo_clique_aba: Option<(usize, Instant)>,
    nome_aba: String,
    linhas_grade: bool,
    /// Destaque apenas visual dos eixos da célula ativa (não persiste no FSHZ).
    status: String,
    carregando: bool,
    /// Há célula visível esperando conta: o pulso de cálculo fica ligado.
    calculo_pendente: bool,
    /// Aviso transitório na própria barra de abas, não uma linha de rodapé.
    aviso_abas: Option<(String, Instant)>,
    menu_aberto: bool,
    /// Lista do menu clássico aberta (faixa recolhida).
    barra_menu_aberta: bool,
    /// Tela inicial em janela inteira: fica ativa até escolher nova planilha, modelo, abrir ou recente.
    em_tela_inicio: bool,
    /// Passo do modo de captura (0 = preparar, 1 = fotografar, 2 = feito).
    passo_captura: u8,
    /// Modo de captura: o clique de FEATHERSHEET_CAPTURAR_CLIQUE já foi dado.
    captura_clicou: bool,
    /// Primeiro modelo visível no carrossel da galeria e modo "ver todos".
    carrossel_modelos: usize,
    ver_todos_modelos: bool,
    /// Filtro da lista de recentes da tela inicial (nome ou pasta).
    busca_recentes: String,
    menu_pagina: menu_principal::Pagina,
    recentes: menu_principal::Historico,
    /// Caminho congelado antes da confirmação de salvamento, independente da
    /// reordenação da lista de recentes.
    recente_destino: Option<PathBuf>,
    menu_grupo: usize,
    menu_caminho: Vec<usize>,
    menu_cursor: usize,
    contexto: Option<(Contexto, f32, f32)>,
    contexto_tamanho: (f32, f32),
    /// Submenu aberto no menu de contexto (Colar especial ▸, Inserir ▸).
    contexto_sub: Option<u8>,
    reexibir_abas_aberto: bool,
    ponteiro_abas_x: f32,
    paleta_aba_aberta: bool,
    busca_comandos: String,
    categoria_funcoes: usize,
    catalogos_funcoes: Vec<Vec<String>>,
    mostrar_fx: bool,
    /// Apenas exibição: mostra =fórmulas brutas, sem alterar valores do motor.
    mostrar_formulas: bool,
    cor_ativa: Option<AlvoCor>,
    cor_hsva: [f32; 4],
    cor_hex: String,
    faixa_ativa: AbaFaixa,
    formulas_menu: Option<formulas_ui::MenuFormulas>,
    /// Onde o menu de Fórmulas abriu: fica parado, não segue o mouse.
    formulas_menu_x: f32,
    formulas_menu_y: f32,
    rastreio: formulas_ui::Rastreio,
    /// Fórmulas → Criar a partir da seleção aberta, com as bordas marcadas.
    /// Fórmulas → Janela de inspeção (vale para a sessão).
    inspecoes: Vec<inspecao_ui::Inspecao>,
    inspecao_selecionada: Option<usize>,
    /// Fórmulas → Avaliar fórmula aberto.
    avaliar: Option<avaliar_ui::Estado>,
    criar_nomes: Option<feathersheet_core::planilha::sheet::nomes_selecao::Bordas>,
    faixa_extras_aberto: bool,
    faixa_grupo_aberto: Option<usize>,
    config_secao: usize,
    atalho_captura: Option<&'static str>,
    auto_salvar_desde: Option<Instant>,
    /// Duração da última gravação para ajustar frequência do autosave.
    auto_salvar_ultimo_custo_ms: u64,
    pendente: Option<Pendente>,
    /// Há uma gravação de .fshz rodando em segundo plano.
    salvando: bool,
    /// O que fazer quando a gravação em andamento terminar (fechar, abrir…).
    apos_gravar: Option<Pendente>,
    busca: dados::Busca,
    validacao_formulario: Option<validacao_ui::Formulario>,
    erro_explicando: Option<(usize, u32, u32)>,
    validacao_erro: Option<String>,
    validacao_editando: Option<usize>,
    validacao_aba: validacao_ui::AbaValidacao,
    /// Alerta de erro aberto pela digitação de um valor inválido.
    validacao_alerta: Option<validacao_dialogo::AlertaPendente>,
    /// Sim/OK do alerta: o próximo `confirmar` grava sem validar.
    validacao_aceita: bool,
    /// Circular dados inválidos: aba e células circuladas.
    validacao_circulos: Option<(usize, Vec<(u32, u32)>)>,
    /// Diálogo Preencher > Série.
    serie: Option<serie_ui::Estado>,
    lista_aberta: bool,
    /// Alt+↓: valores da coluna para escolher (a validação tem preferência).
    lista_coluna: Option<Vec<String>>,
    lista_pagina: usize,
    /// Item da lista suspensa marcado por ↑/↓.
    lista_destaque: usize,
    cabecalho_dados: bool,
    assistente_divisao: Option<assistente_divisao::Estado>,
    divisao_personalizada: bool,
    divisao_outro: String,
    macro_codigo: pelicano::Content,
    macro_nome: String,
    macro_catalogo: feather_macros::Catalogo,
    macro_visual: macros::EstadoVisual,
    macro_saida: String,
    botao_macro_modal: Option<botoes_macro::Editor>,
    botao_macro_selecionado: Option<(u32, String)>,
    arraste_botao_macro: Option<botoes_macro::Arraste>,
    area_copiada: Option<AreaCopiada>,
    /// Área do último Ctrl+C (aba, retângulo): a grade marca com tracejado,
    /// como no Excel, até Esc, outra cópia ou uma edição (17/10/2026).
    marca_copia: Option<(usize, Rect)>,
    colar_menu_aberto: bool,
    edicao_menu: Option<&'static str>,
    edicao_popup_x: f32,
    edicao_popup_y: f32,
    bordas_menu_aberto: bool,
    bordas_popup_x: f32,
    bordas_popup_y: f32,
    orientacao_menu_aberto: bool,
    orientacao_popup_x: f32,
    orientacao_popup_y: f32,
    formato_celula: Option<formatar_celulas::Estado>,
    pincel_formatacao: Option<PincelFormatacao>,
    borda_desenho: desenho_borda_ui::Estado,
    /// O F12 inteiro mora no `feather-ui`; aqui só se alimenta o console e
    /// as métricas.
    monitor: feather_ui::monitor::Monitor,
    /// Último painel aberto/fechado por atalho e quando: no X11/XWayland a
    /// tecla segurada repete e a troca de foco reenvia a tecla apertada (o
    /// Iced 0.13 não filtra), e o F12 abria e fechava o monitor várias vezes.
    ultima_alternancia: Option<(Auxiliar, std::time::Instant)>,
    ajuda_estado: feather_ui::ajuda::Estado,
    /// Passo aberto na janela Primeiros passos.
    primeiros_passos_passo: usize,
    /// Versão escolhida na lateral de O que há de novo.
    novidades_versao: usize,
    /// Documento já avisado de que o salvamento automático não o cobre.
    aviso_salvamento_de: Option<String>,
    /// Faixa acima da grade: o documento não tem cópia automática
    /// (texto, e se é por ser grande demais).
    faixa_salvamento: Option<(String, bool)>,
    /// Depois de exportar .xlsx: funções usadas que o Excel não tem.
    aviso_xlsx: Option<Vec<String>>,
    ctrl_pressionado: bool,
    /// Versão da aba de origem na última conta de cada dinâmica com
    /// "Atualizar sozinha" (pelo id da dinâmica).
    versoes_origem_dinamicas: std::collections::HashMap<String, u32>,
    /// Origens citadas pelas fórmulas e quem assina este arquivo (vinculos.rs).
    vinculos: vinculos::Estado,
    /// Segmentações de dados das dinâmicas: escolha do campo e cartões.
    segmentacao: dinamica_ui::Segmentacoes,
    /// O último título mandado a cada janela (o pelicano só troca quando pedido).
    titulos: RefCell<std::collections::HashMap<WindowId, String>>,
    /// O evento em andamento passou pelo `on_unhandled_event` (nenhum widget o usou).
    evento_livre: std::cell::Cell<bool>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            versoes_origem_dinamicas: Default::default(),
            vinculos: Default::default(),
            segmentacao: Default::default(),
            titulos: Default::default(),
            evento_livre: Default::default(),
            graficos: graficos::Estado::default(),
            dinamica: None,
            tabela_area: None,
            tabela_nome: String::new(),
            tabela_cabecalho: false,
            tabela_branca: false,
            tabela_editando: None,
            tabela_nome_novo: String::new(),
            tabela_renomeando: false,
            tabela_redimensionando: false,
            tabela_intervalo_novo: String::new(),
            tabela_operador: "contains",
            tabela_criterio: String::new(),
            tabela_col_filtro: 0,
            tabela_filtro_lista: Vec::new(),
            tabela_filtro_vistos: HashSet::new(),
            tabela_filtro_criterio_valores: HashSet::new(),
            tabela_filtro_indices: Vec::new(),
            tabela_filtro_marcados: HashSet::new(),
            tabela_filtro_truncado: false,
            tabela_filtro_busca: String::new(),
            tabela_filtro_inicio_visivel: 0,
            tabela_filtro_proxima_linha: 0,
            tabela_filtro_ultima_linha: 0,
            tabela_filtro_automatico: false,
            tabela_filtro_geracao: 0,
            tabela_filtro_original: None,
            tabela_filtro_editado: false,
            tabela_popup_filtro: false,
            tabela_popup_totais: false, barra_status_menu: false, mcp_estado: String::new(),
            tabela_popup_x: 0.0,
            tabela_popup_y: 0.0,
            tabela_filtro_pagina: 0,
            tabela_filtro_extra: Default::default(),
            tamanho_principal: Size::new(1280.0, 800.0),
            efeitos_imagem: efeitos_imagem::Estado::default(),
            imagens_importadas: imagens_importadas::Cache::default(),
            imagem_selecionada: None,
            imagem_contexto: None,
            formatar_imagem: formatar_imagem::Estado::default(),
            dinamica_fundo: Default::default(),
            arraste_imagem: None,
            condicional: RefCell::new(condicional::Estado::default()),
            perfil_coluna: None, perfil_geracao: 0,
            estatisticas: None, estatisticas_geracao: 0,
            openquery: None,
            nomes_modal: None,
            link_modal: None,
            dimensao_modal: None,
            celulas_modal: None,
            subtotal: None,
            vba_pendente: None, macro_pergunta: None, formulario_macro: None, macro_valores: None, historico_macros: Default::default(), macro_mensagens: Vec::new(), eventos_vba: Vec::new(), macros_bloqueadas: false, aviso_macros: false, macro_rodou: false, profundidade_update: 0,
            protecao_modal: None,
            comentario_editor: None, comentario_sobre: None, comentarios_todos: false,
            novo_estilo: None,
            colar_especial: None,
            selecoes_extras: Vec::new(),
            ir_para: None,
            classificar: None,
            atingir_meta: None,
            tabela_dados: None,
            tabela_dados_ultima: None,
            cenarios: None,
            colar_especial_ultimo: Default::default(),
            congelados: std::collections::HashMap::new(),
            grade_oculta: std::collections::HashSet::new(),
            congelar_menu_aberto: false,
            tinta: Default::default(),
            objetos: Default::default(),
            congelar_popup: (8., 120.),
            condicional_aberto: false, condicional_intervalo: String::new(),
            condicional_pegando: false, condicional_so_selecao: false,
            condicional_cor_aberta: None,
            condicional_hsva: [0., 0., 0., 1.],
            condicional_hex: String::new(),
            condicional_menu_aberto: false,
            condicional_menu_x: 8.0,
            condicional_menu_y: 132.0,
            tabelas_menu_aberto: false, estilos_menu_aberto: false,
            estilos_popup_x: 520.0, estilos_popup_y: 236.0,
            tabela_estilo_pendente: None,
            ponteiro_x: 150.0,
            ponteiro_y: 120.0,
            condicional_editando: None,
            condicional_operador: TipoCondicional::Maior,
            condicional_valor: "10".into(),
            condicional_valor2: "20".into(),
            condicional_cor: "#fce8b2".into(),
            condicional_cor_texto: "".into(),
            condicional_cor_min: "#fff2b2".into(),
            condicional_cor_meio: "#ffffff".into(),
            condicional_cor_negativa: "#d93025".into(),
            condicional_ponto_meio: false,
            condicional_ocultar_valor: false,
            condicional_negrito: false,
            condicional_italico: false,
            condicional_sublinhado: false,
            condicional_riscado: false,
            workbook: Workbook::new(), eixos_grade: RefCell::new(Default::default()), duplicados:None, impressao: Default::default(), resumo_selecao: RefCell::new(status_selecao::Estado::default()), atualizacao: Default::default(), cache_grade: Default::default(),
            config: Configuracao::default(),
            janelas: Janelas::default(), ultimo_clique_barra: None,
            caminho: None, recuperacao: Default::default(), origem: None, alterado: false, tela_inteira: false, folha: 0,
            linha: 0, coluna: 0, fim_selecao: (0, 0), linha_inicio: 0, coluna_inicio: 0,
            ancorar_rodape: false,
            edicao: String::new(), edicao_inline: None, multilinha: None, editor_formula:EditorFormula::default(), endereco: "A1".into(), endereco_digitado: false,
            aba_em_renomeacao:None, erro_nome_aba:None, ultimo_clique_aba:None,
            editando_nome_aba:false, nome_aba: "Planilha1".into(), linhas_grade: true,
            status: tr!("Pronto — FeatherSheet nativo Rust/Iced").into(),
            carregando: false, calculo_pendente: false,
            aviso_abas: None,
            menu_aberto: false, barra_menu_aberta: false, menu_pagina: menu_principal::Pagina::Inicio,
            em_tela_inicio: false, passo_captura: 0, captura_clicou: false, carrossel_modelos: 0, ver_todos_modelos: false, busca_recentes: String::new(),
            recentes: menu_principal::Historico::carregar(),
            recente_destino: None,
            menu_grupo: 0, menu_caminho: Vec::new(), menu_cursor: 0,
            contexto: None,
            contexto_sub: None,
            ponteiro_abas_x: 32.0,
            paleta_aba_aberta: false,
            contexto_tamanho: (
                crate::geometria_overlay::LARGURA_CONTEXTUAL,
                crate::geometria_overlay::ALTURA_CONTEXTUAL,
            ),
            reexibir_abas_aberto: false,
            busca_comandos: String::new(), categoria_funcoes: 0,
            catalogos_funcoes: faixa_nativa::montar_catalogos_funcoes(), mostrar_fx: true,
            mostrar_formulas: false,
            cor_ativa: None, cor_hsva: [0.0, 1.0, 1.0, 1.0], cor_hex: "#000000".into(),
            faixa_ativa: AbaFaixa::Inicio,
            formulas_menu: None, formulas_menu_x: 8.0, formulas_menu_y: 140.0, rastreio: formulas_ui::Rastreio::default(), criar_nomes: None, avaliar: None, inspecoes: Vec::new(), inspecao_selecionada: None,
            faixa_extras_aberto: false, faixa_grupo_aberto: None,
            config_secao: 0, atalho_captura: None, auto_salvar_desde: None, auto_salvar_ultimo_custo_ms: 0, pendente: None, salvando: false, apos_gravar: None, busca: dados::Busca::default(), validacao_formulario: None, erro_explicando: None, validacao_erro: None, validacao_editando: None, validacao_aba: Default::default(), validacao_alerta: None, validacao_aceita: false, validacao_circulos: None, serie: None, lista_aberta: false, lista_coluna: None, lista_pagina: 0, lista_destaque: 0, cabecalho_dados: false,
            assistente_divisao: None,
            divisao_personalizada: false, divisao_outro: String::new(),
            macro_codigo: pelicano::Content::with_text("// Execute apenas quando desejar alterar a planilha.\nlog(sheet.range(\"A1\").value);"),
            macro_nome: "Macro1".into(),
            // As macros são de cada pasta (vêm do arquivo aberto); nada global.
            macro_catalogo: feather_macros::Catalogo::default(),
            macro_visual: macros::EstadoVisual::default(),
            macro_saida: String::new(),
            botao_macro_modal: None,
            botao_macro_selecionado: None,
            arraste_botao_macro: None,
            area_copiada: None,
            marca_copia: None,
            colar_menu_aberto: false,
            edicao_menu: None,
            edicao_popup_x: 1120.0,
            edicao_popup_y: 235.0,
            bordas_menu_aberto: false, bordas_popup_x: 240.0, bordas_popup_y: 160.0,
            orientacao_menu_aberto: false, orientacao_popup_x: 360.0, orientacao_popup_y: 160.0,
            formato_celula: None,
            pincel_formatacao: None,
            borda_desenho: Default::default(),
            monitor: feather_ui::monitor::Monitor::novo("FeatherSheet"),
            ultima_alternancia: None,
            ajuda_estado: Default::default(),
            primeiros_passos_passo: 0,
            novidades_versao: 0,
            aviso_salvamento_de: None,
            faixa_salvamento: None,
            aviso_xlsx: None,
            ctrl_pressionado: false,
        }
    }
}

impl App {
    /// Troca a Ribbon e guarda somente as abas fixas; seleção contextual
    /// (Tabela) não altera a última preferência da pessoa.
    fn selecionar_faixa(&mut self, aba: AbaFaixa) {
        self.faixa_ativa = aba;
        if let Some(chave) = aba.chave_persistencia() {
            if self.config.faixa_aba != chave {
                self.config.faixa_aba = chave.to_owned();
                self.config.gravar();
            }
        }
    }

    pub fn novo() -> (Self, Task<Msg>) {
        Self::novo_com(std::env::args_os().nth(1).map(PathBuf::from))
    }

    /// Sem arquivo na linha de comando, a janela abre na tela inicial; com arquivo, vai direto à planilha.
    fn novo_com(inicial: Option<PathBuf>) -> (Self, Task<Msg>) {
        let mut app = Self { config: Configuracao::carregar(), em_tela_inicio: inicial.is_none(), ..Default::default() };
        app.recuperacao.orfas = recuperacao::procurar_orfas();
        // A vigia de memória da suíte segue o limite das Configurações.
        feathersheet_io::memoria::definir_limite_gb(app.config.limite_memoria_gb);
        feathersheet_core::paralelismo::definir(app.config.nucleos as usize);
        feathersheet_core::set_locale_config(app.regionalizacao());
        ajuda::definir_idioma(&app.config.idioma);
        // Modo de captura: FEATHERSHEET_CAPTURAR_IDIOMA=en-US mostra a tela
        // nesse idioma sem gravar nada na configuração do usuário.
        if std::env::var_os("FEATHERSHEET_CAPTURAR").is_some() {
            if let Ok(idioma) = std::env::var("FEATHERSHEET_CAPTURAR_IDIOMA") {
                ajuda::definir_idioma(&idioma);
            }
        }
        // A pasta em branco nasce com o nome de aba do idioma ("Sheet1").
        app.workbook = Self::pasta_nova();
        app.nome_aba = app.workbook.nomes().first().cloned().unwrap_or_default();
        app.linhas_grade = app.config.linhas_grade;
        app.mostrar_fx = app.config.barra_formulas;
        app.mostrar_formulas = app.config.mostrar_formulas;
        app.faixa_ativa = AbaFaixa::de_chave_persistencia(&app.config.faixa_aba);
        definir_tema(escuro_resolvido(app.config.tema_auto, app.config.escuro), app.config.acento);
        feather_ui::marca::definir_marca(feather_ui::marca::Produto::FeatherSheet,
            app.config.logo_usa_acento());
        // Um XLSX/CSV grande não deve travar a criação da janela.
        if inicial.is_some() {
            app.carregando = true;
            app.status = tr!("Carregando planilha em segundo plano…").into();
            app.monitor.registrar(Nivel::Info, tr!("Arquivo"), &app.status);
        }
        // Ícone e app_id de todas as janelas: sem o app_id o Linux não liga a janela ao
        // feathersheet.desktop instalado (sem nome nem ícone, em "Processos do Sistema").
        if let Ok(png) = image::load_from_memory(include_bytes!("../../../pacote/icones/feathersheet-256.png")) {
            let rgba = png.to_rgba8();
            let (l, a) = rgba.dimensions();
            if let Ok(icone) = pelicano::Icon::from_rgba(rgba.into_raw(), l, a) {
                feather_ui::marca::definir_icone_janela(icone);
            }
        }
        // A principal nasce escondida (`start_hidden` no main) e abre aqui, com a moldura
        // que a configuração pede.
        let id = WindowId::MAIN;
        let task = Task::open_window(id, feather_ui::marca::preparar_janela(pelicano::Settings {
            title: app.titulo(id),
            size: std::env::var("FEATHERSHEET_CAPTURAR_TAMANHO").ok()
                .and_then(|t| t.split_once('x').and_then(|(w, h)| Some(Size::new(w.parse().ok()?, h.parse().ok()?))))
                .unwrap_or(Size::new(1280., 800.)),
            min_size: Some(Size::new(760., 520.)),
            decorations: !app.config.barra_propria, close_on_request: false,
            ..Default::default()
        }));
        app.janelas.principal = Some(id);
        definir_janela_principal(id);
        // MCP ao vivo só se a pessoa ligou nas Configurações (desligado de fábrica).
        mcp_vivo::somente_leitura(app.config.mcp_so_leitura);
        if app.config.mcp_ativo {
            app.mcp_estado = mcp_vivo::ligar(true);
            app.monitor.registrar(Nivel::Info, "MCP", &app.mcp_estado);
        }
        // Primeira vez ou recém-atualizado: Primeiros passos / O que há de
        // novo abrem por cima, depois da janela principal.
        // `batch`, nunca `chain`: no Iced 0.13, `chain` com tarefa vazia
        // DESCARTA a primeira, e a janela principal não abriria.
        let boas_vindas = app.janela_de_boas_vindas().map_or_else(Task::none, |aux| app.alternar(aux));
        let janela = Task::batch([task, Task::done(Msg::AuxAberta), boas_vindas, app.ligar_caneta(id)]);
        let tarefas = if let Some(p) = inicial {
            Task::batch([janela, Self::carregar_em_background(p, app.config.separador_csv())])
        } else {
            janela
        };
        (app, tarefas)
    }

    /// Executa leitura e decodificação em worker, nunca no thread Iced.
    fn carregar_em_background(path: PathBuf, separador: Option<char>) -> Task<Msg> {
        Task::perform(async move {
            match tokio::task::spawn_blocking(move || {
                let inicio = Instant::now();
                feathersheet_io::progresso::comecar(&path);
                let (workbook, folha) = Self::decodificar_caminho(&path, separador)?;
                Ok::<PastaLida, String>(PastaLida {
                    caminho: path,
                    dados: Arc::new(Mutex::new(Some((workbook, folha)))),
                    ms: inicio.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                })
            }).await {
                Ok(resultado) => resultado.map(Some),
                Err(e) => Err(trf!("Falha ao carregar arquivo: {e}")),
            }
        }, Msg::Aberto)
    }

    pub fn titulo(&self, id: window::Id) -> String {
        match self.janelas.tipo(id) {
            Some(Auxiliar::Configuracoes) => tr!("Configurações do FeatherSheet").into(),
            Some(Auxiliar::Ajuda) => tr!("Ajuda do FeatherSheet").into(),
            Some(Auxiliar::Monitor) => tr!("Monitor do FeatherSheet").into(),
            Some(Auxiliar::Macros) => tr!("Macros do FeatherSheet — Editor de macros · FeatherWorks").into(),
            Some(Auxiliar::FormatarCelulas) => tr!("Formatar Células — FeatherSheet").into(),
            Some(Auxiliar::DividirTexto) => tr!("Converter texto em colunas — FeatherSheet").into(),
            Some(Auxiliar::RemoverDuplicados) => tr!("Remover duplicados — FeatherSheet").into(),
            Some(Auxiliar::Imprimir) => tr!("Imprimir — FeatherSheet").into(),
            Some(Auxiliar::OpenQuery) => tr!("OpenQuery — Editor de consultas · FeatherWorks").into(),
            Some(Auxiliar::Inspecao) => tr!("Janela de inspeção — FeatherSheet").into(),
            Some(Auxiliar::PrimeirosPassos) => tr!("Primeiros passos — FeatherSheet").into(),
            Some(Auxiliar::Novidades) => tr!("O que há de novo — FeatherSheet").into(),
            Some(Auxiliar::Condicional) => tr!("Formatação condicional — FeatherSheet").into(),
            Some(Auxiliar::Formulario) => self.formulario_macro.as_ref()
                .map_or_else(|| tr!("Formulário").to_string(), |f| if f.form.titulo.trim().is_empty() { f.form.nome.clone() } else { f.form.titulo.clone() }),
            None => format!("{}{} — FeatherSheet",
                if self.alterado { "* " } else { "" }, self.nome_documento()),
        }
    }
    pub fn tema(&self, _id: window::Id) -> pelicano::Theme { tema() }
    fn nome_documento(&self) -> String {
        self.caminho.as_ref().or(self.origem.as_ref()).and_then(|p| p.file_name()).map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| tr!("Sem título.fshz").into())
    }
    /// O nome sem extensão que os diálogos de salvar/exportar sugerem.
    pub(super) fn nome_base_documento(&self) -> String {
        self.caminho.as_ref().or(self.origem.as_ref()).and_then(|p| p.file_stem())
            .map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| tr!("Sem título").into())
    }
    fn acoes(id: window::Id) -> cromo::AcoesJanela<Msg> {
        cromo::AcoesJanela {
            arrastar: Msg::Arrastar(id), minimizar: Msg::Minimizar(id),
            maximizar: Msg::Maximizar(id), fechar: Msg::Fechar(id),
        }
    }
    /// O mesmo painel alternado há menos de meio segundo: é a tecla repetida
    /// ou reenviada pela troca de foco, não um novo pedido. Registra o pedido.
    fn alternancia_repetida(&mut self, tipo: Auxiliar) -> bool {
        let agora = std::time::Instant::now();
        let repetida = self.ultima_alternancia
            .is_some_and(|(t, quando)| t == tipo && agora.duration_since(quando) < std::time::Duration::from_millis(500));
        if !repetida { self.ultima_alternancia = Some((tipo, agora)); }
        repetida
    }

    fn alternar(&mut self, tipo: Auxiliar) -> Task<Msg> {
        if self.alternancia_repetida(tipo) { return Task::none(); }
        let tam = Self::tamanho_aux(tipo);
        self.janelas.alternar::<Msg>(tipo, tam, self.config.barra_propria).map(|_| Msg::AuxAberta)
    }

    /// O tamanho com que cada janela auxiliar abre.
    pub(super) fn tamanho_aux(tipo: Auxiliar) -> Size {
        match tipo {
            Auxiliar::Configuracoes => Size::new(920., 700.),
            Auxiliar::Ajuda => Size::new(1190., 760.),
            Auxiliar::Monitor => Size::new(980., 620.),
            Auxiliar::Macros => Size::new(1180., 760.),
            Auxiliar::FormatarCelulas => Size::new(780., 580.),
            Auxiliar::DividirTexto => Size::new(800., 610.),
            Auxiliar::RemoverDuplicados => Size::new(640., 560.),
            Auxiliar::Imprimir => Size::new(1000., 680.),
            Auxiliar::OpenQuery => Size::new(1280., 800.),
            Auxiliar::Inspecao => Size::new(820., 360.),
            Auxiliar::PrimeirosPassos | Auxiliar::Novidades => Size::new(1100., 720.),
            Auxiliar::Condicional => Size::new(900., 620.),
            Auxiliar::Formulario => Size::new(392., 292.),
        }
    }
    /// O objeto selecionado sobre a grade (forma ou ícone, gráfico, imagem
    /// ou botão de macro), para saber quando a seleção mudou.
    fn objeto_na_mao(&self) -> Option<String> {
        if let Some((aba, id)) = self.objetos.selecionado { return Some(format!("forma {aba} {id}")); }
        if let Some(id) = self.grafico_selecionado_id() { return Some(format!("grafico {id}")); }
        if let Some((aba, id)) = &self.imagem_selecionada { return Some(format!("imagem {aba} {id}")); }
        self.botao_macro_selecionado.as_ref().map(|b| format!("botao {b:?}"))
    }

    /// Pasta em branco com a primeira aba no idioma da interface: "Planilha1",
    /// "Sheet1" ou "Hoja1", como o Excel de cada idioma.
    pub(super) fn pasta_nova() -> Workbook {
        let mut pasta = Workbook::new();
        let nome = format!("{}1", tr!("Planilha"));
        pasta.editar(0, |s| s.name = nome);
        pasta
    }

    /// Quais painéis laterais estão abertos: gráfico, dinâmica (campos) e
    /// imagem.
    fn paineis_laterais(&self) -> [bool; 3] {
        [
            self.graficos.aberto,
            self.dinamica_lateral_aberta(),
            self.formatar_imagem.aberto,
        ]
    }

    /// Só um painel lateral por vez (17/10/2026: a dinâmica abria por cima do
    /// painel do gráfico): o que acabou de abrir fecha os outros, pelo mesmo
    /// caminho do botão de fechar de cada um.
    fn um_painel_lateral_por_vez(&mut self, antes: [bool; 3]) {
        let agora = self.paineis_laterais();
        let Some(novo) = (0..3).find(|&i| agora[i] && !antes[i]) else { return };
        for i in (0..3).filter(|&i| i != novo && agora[i]) {
            let _ = match i {
                0 => self.update(Msg::Grafico(graficos::Evento::Fechar)),
                1 => self.update(Msg::Dinamica(dinamica_ui::Evento::Fechar)),
                _ => self.update(Msg::FormatarImagem(formatar_imagem::Evento::Fechar)),
            };
        }
    }

    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        let msg = match msg {
            Msg::Lote(lote) => return Task::batch(lote.into_iter().map(|m| self.update(m)).collect::<Vec<_>>()),
            msg => msg,
        };
        // Toda mensagem pode mudar o que a grade mostra, menos as desta lista: o pulso
        // da soma do rodapé roda centenas de vezes numa seleção grande e só mexe no
        // rodapé. O tique dos vínculos só publica e confere origens (o que muda
        // valor chega em outra mensagem). Na dúvida, fica fora da lista
        // (redesenha como antes).
        if !matches!(msg, Msg::ResumoPulso | Msg::Vinculos(vinculos::Evento::Pulso)) {
            self.cache_grade.desenho.clear();
        }
        // Enter com o foco no campo fx (sem editar) depois de apontar para
        // outra janela: confirma lá, como a tecla Enter na grade.
        if self.profundidade_update == 0 && matches!(msg, Msg::EnviarEditor) && self.tecla_para_quem_aponta(true) {
            self.status = tr!("Fórmula da outra janela confirmada.").into();
            return Task::none();
        }
        let selecao_antes = (self.folha, self.linha, self.coluna, self.fim_selecao);
        // Só a chamada de fora compara o antes e o depois para os eventos do VBA.
        let antes = if self.profundidade_update == 0 { self.antes_dos_eventos() } else { None };
        let na_mao = if self.profundidade_update == 0 { Some(self.objeto_na_mao()) } else { None };
        let paineis = (self.profundidade_update == 0).then(|| self.paineis_laterais());
        // Só a chamada de fora: as de dentro já contam no tempo dela.
        let vigia = (self.profundidade_update == 0).then(|| (std::time::Instant::now(), crate::vigia::nome_curto(&msg)));
        self.profundidade_update += 1;
        let mut tarefa = self.update_sem_eventos(msg);
        self.profundidade_update -= 1;
        if let Some((inicio, nome)) = vigia {
            crate::vigia::conferir(|| nome, inicio.elapsed());
        }
        // Forma, ícone, gráfico ou imagem recém-selecionados: o campo fx
        // solta o foco, senão ele engole o Delete (e as setas) que são do objeto.
        if let Some(antes) = na_mao {
            let agora = self.objeto_na_mao();
            if agora.is_some() && agora != antes {
                tarefa = Task::batch([tarefa, crate::campo_fx::desfocar()]);
            }
        }
        if let Some(antes) = antes { self.disparar_eventos(antes); }
        if let Some(antes) = paineis { self.um_painel_lateral_por_vez(antes); }
        if self.profundidade_update == 0 { self.cuidar_apontar(selecao_antes); }
        // O rodapé decide aqui se vai somar em lotes: decidido só no desenho,
        // o pulso de 16 ms só nascia no tique seguinte do monitor (até 1 s).
        if self.profundidade_update == 0 { self.sincronizar_resumo(); }
        // As linhas do histórico de macros entram no Console só no fim: no
        // meio do update elas fariam a saída da macro parecer já registrada.
        if self.profundidade_update == 0 {
            let linhas = self.historico_macros.tirar_console();
            if !linhas.is_empty() {
                for (nivel, linha) in linhas { self.monitor.registrar(nivel, tr!("Macros"), &linha); }
                return Task::batch([tarefa, self.monitor.acompanhar()]);
            }
        }
        tarefa
    }

    fn update_sem_eventos(&mut self, msg: Msg) -> Task<Msg> {
        let anterior = self.status.clone();
        let revisao = self.monitor.console.revisao();
        let repetir_exclusao = matches!(msg, Msg::RemoverAba);
        let arquivo_erro = matches!(&msg, Msg::Aberto(Err(_)));
        let arquivo_aberto = matches!(&msg, Msg::Aberto(Ok(Some(_))));
        let arquivo_fim = matches!(&msg, Msg::Aberto(_));
        let repetir_macro = matches!(&msg, Msg::MacroExecutar);
        let observar_macro = matches!(&msg,
            Msg::MacroExecutar | Msg::MacroNovoModulo | Msg::MacroGuardarModulo | Msg::MacroAbrirModulo(_)
            | Msg::MacroExcluirModulo | Msg::MacroCatalogoLido(_) | Msg::MacroDestinoCatalogo(_)
            | Msg::MacroLido(_) | Msg::MacroDestino(_) | Msg::MacroTeclado(..) | Msg::Teclado(..));
        let saida_anterior = observar_macro.then(|| self.macro_saida.clone());
        let edicao_para_autosave = matches!(&msg,
            Msg::Editar(_) | Msg::Digitar(_) | Msg::EditarMultilinha | Msg::InserirQuebraLinha | Msg::Confirmar | Msg::EnviarEditor |
            Msg::Colar(_) | Msg::ColarValores(_) | Msg::ColarFormato(_) |
            Msg::ColarEspecial(..) | Msg::Limpar | Msg::Desfazer | Msg::Refazer |
            Msg::Formatar(_) | Msg::Estrutura(_) | Msg::TabelaAcao(_) |
            Msg::AlternarEstilo(_) | Msg::CondicionalAdicionar | Msg::CondicionalRemover(_)
            | Msg::Ordenar(_) | Msg::OrdenarPagina(_) | Msg::Preencher(_) | Msg::PreencherAlca(_) | Msg::PreenchimentoRelampago | Msg::Serie(_) | Msg::CaixasInserir | Msg::CaixaAlternar(..) | Msg::MoverAba(..)
            | Msg::OcultarDimensoes(_) | Msg::ReexibirDimensoes(_)
            | Msg::MesclarModo(_) | Msg::AjustarAltura | Msg::Dimensao(_)
            | Msg::LimparTudo | Msg::AlterarRecuo(_) | Msg::AplicarPincel | Msg::LimparHiperlinks);
        let paineis_antes = (self.graficos.aberto, self.dinamica.is_some());
        // Aba protegida: o portão recusa ou desfaz o que não pode (protecao_ui).
        let tarefa = self.processar_protegido(msg);
        // Um painel lateral por vez: o que acabou de abrir fecha o outro.
        if self.graficos.aberto && self.dinamica.is_some() {
            if !paineis_antes.1 { self.graficos.aberto = false; }
            else if !paineis_antes.0 { self.dinamica = None; }
        }
        // Pasta nova ou aberta nasce automática no motor; vale a preferência.
        self.aplicar_modo_calculo();
        if !self.inspecoes.is_empty() { self.limpar_inspecoes(); }
        // Depois de qualquer mensagem: o que ficou visível e sem conta é
        // calculado aqui, com orçamento, nunca dentro do desenho.
        self.calcular_visiveis(calculo_visivel::ORCAMENTO);
        if arquivo_aberto { self.em_tela_inicio = false; }
        if !self.alterado { self.auto_salvar_desde = None; }
        else if edicao_para_autosave || self.auto_salvar_desde.is_none() {
            self.auto_salvar_desde = Some(Instant::now());
        }
        let registrado_internamente = self.monitor.console.revisao() != revisao;
        let mudou_status = (self.status != anterior || repetir_exclusao || arquivo_fim)
            && !self.status.is_empty();
        if mudou_status {
            self.aviso_abas = Some((self.status.clone(), Instant::now()));
            // Um atalho pode despachar update novamente; o evento interno já registra.
            // O resultado de abrir entra sempre: as métricas do motor que o
            // Aberto recolhe sobem a revisão e escondiam o erro do arquivo.
            if self.monitor.console.revisao() == revisao || arquivo_fim {
                let nivel = if arquivo_erro { Nivel::Erro }
                    else { Nivel::da_mensagem(&self.status) };
                self.monitor.registrar(nivel, if arquivo_fim { tr!("Arquivo") } else { tr!("Aplicativo") }, &self.status);
            }
        }
        if let Some(anterior) = saida_anterior {
            if (anterior != self.macro_saida || repetir_macro) && !registrado_internamente {
                self.monitor.console.registrar_saida(tr!("Macros"), &self.macro_saida);
            }
        }
        if self.monitor.console.revisao() != revisao {
            Task::batch([tarefa, self.monitor.acompanhar()])
        } else { tarefa }
    }

    pub fn subscription(&self) -> Subscription<Msg> {
        // O `map` do pelicano não aceita closure que captura: uma por mensagem.
        let a_cada = |ms: u64| Subscription::every(Duration::from_millis(ms));
        Subscription::batch([
            mcp_vivo::assinatura(),
            vinculos::assinatura(self),
            // Caneta: toques lidos pelo XInput2 e contagem dos cliques.
            self.assinatura_caneta(),
            // Dinâmica grande: a leitura da origem anda aos pedaços, um por pulso.
            if self.dinamica_fundo.ativo() {
                a_cada(16).map(|_| Msg::Dinamica(dinamica_ui::Evento::FundoPasso))
            } else { Subscription::none() },
            // Libera a thread da UI após cada lote, inclusive com 1M+ linhas.
            // Quando não existe conversão ativa, não mantém timer adicional.
            if self.assistente_divisao.as_ref()
                .is_some_and(|estado| estado.conversao_em_andamento()) {
                a_cada(12).map(|_| Msg::AssistenteDivisao(EventoAssistenteDivisao::ProcessarLote))
            } else { Subscription::none() },
            a_cada(1000).map(|_| Msg::MonitorPulso),
            if self.marca_copia.is_some() {
                a_cada(100).map(|_| Msg::CopiaPulso)
            } else { Subscription::none() },
            if self.passo_captura < 2 && std::env::var_os("FEATHERSHEET_CAPTURAR").is_some() {
                a_cada(1500).map(|_| Msg::CapturarTela)
            } else { Subscription::none() },
            if self.calculo_pendente {
                a_cada(16).map(|_| Msg::CalcularPulso)
            } else { Subscription::none() },
            if self.carregando {
                a_cada(16).map(|_| Msg::ProgressoPulso)
            } else { Subscription::none() },
            if self.grafico_em_transicao() {
                // Transição da legenda do gráfico: um quadro por tela.
                a_cada(16).map(|_| Msg::Grafico(graficos::Evento::QuadroTransicao))
            } else { Subscription::none() },
            if self.resumo_selecao.borrow().pendente {
                // 8 ms de soma a cada 16 ms: a tela continua a 60 quadros.
                a_cada(16).map(|_| Msg::ResumoPulso)
            } else { Subscription::none() },
        ])
    }

    /// Os eventos da janela `id` (o `event::listen_with` do Iced). `livre`: nenhum widget usou
    /// o evento (o `Status::Ignored` do Iced). Um evento pode virar mais de uma mensagem.
    fn evento_janela(&self, id: window::Id, ev: &Event, livre: bool) -> Option<Msg> {
        use keyboard::{key::Named, Key};
        let mut msgs = Vec::new();
        match ev {
            // Arrastando um campo da tabela dinâmica ou a janela de Localizar: o mouse da
            // janela toda, para seguir o cursor e soltar mesmo fora do painel.
            Event::CursorMoved(p) => {
                if self.dinamica.as_ref().is_some_and(|e| e.arrastando()) {
                    msgs.push(Msg::Dinamica(dinamica_ui::Evento::ArrastoMover(*p)));
                }
                if self.busca.arrasto.is_some() { msgs.push(Msg::BuscaArrastoMover(*p)); }
                // Durante um gesto de forma só o Canvas redesenha (objetos/camada.rs).
                // Só o cursor da janela principal interessa à grade e aos objetos (ver
                // `Msg::CursorJanela`): o de Configurações, Ajuda… não vira posição na planilha.
                if !objetos::arrastando() && eh_janela_principal(id) { msgs.push(Msg::CursorJanela(id, *p)); }
            }
            Event::MousePressed(_) => msgs.extend(self.clique_mouse_caneta()),
            Event::MouseReleased(pelicano::MouseButton::Left) => {
                if self.dinamica.as_ref().is_some_and(|e| e.arrastando()) {
                    msgs.push(Msg::Dinamica(dinamica_ui::Evento::ArrastoSoltar));
                }
                if self.busca.arrasto.is_some() { msgs.push(Msg::BuscaArrastoFim); }
                // Divisória do editor de macros: soltar em cima do código (que
                // captura o clique) não chegava ao `on_release` e o arrasto ficava preso.
                if self.macro_visual.arrasto.is_some() {
                    msgs.push(Msg::MacroVisual(macros::EventoVisual::DivisoriaFim));
                }
                msgs.push(Msg::Grafico(graficos::Evento::Soltar));
            }
            // Ctrl deve ser rastreado mesmo quando algum editor captura a tecla.
            Event::Key { key: Key::Named(Named::Control), pressed: true, .. } => msgs.push(Msg::ControleZoom(true)),
            // Alt+Enter precisa alcançar o editor mesmo com o evento capturado
            // pelo TextInput, que não suporta quebras de linha.
            Event::Key { key: key @ Key::Named(Named::Enter), modifiers, pressed: true, .. } if modifiers.alt() =>
                msgs.push(Msg::Teclado(id, key.clone(), *modifiers)),
            Event::Key { key, modifiers, pressed: true, .. } if livre || *key == Key::Named(Named::F4) =>
                msgs.push(Msg::Teclado(id, key.clone(), *modifiers)),
            // Mesmo quando o text_editor consome o evento, F5 e Ctrl+S
            // devem alcançar o editor de macros. Nas outras janelas o
            // handler específico ignora o evento, sem afetar a planilha.
            Event::Key { key, modifiers, pressed: true, .. } if macros::atalhos::deve_ouvir_capturada(key, *modifiers) =>
                msgs.push(Msg::MacroTeclado(id, key.clone(), *modifiers)),
            Event::Key { key, modifiers, pressed: true, .. } if matches!(key, Key::Named(
                Named::ArrowUp | Named::ArrowDown | Named::ArrowLeft | Named::ArrowRight
                | Named::Home | Named::End | Named::F2 | Named::Tab | Named::Escape)) =>
                msgs.push(Msg::TecladoNoCampo(id, key.clone(), *modifiers)),
            Event::Key { key, pressed: false, .. } => msgs.push(Msg::TeclaLiberada(key.clone())),
            // Shift+Enter na busca do editor de macros volta uma ocorrência:
            // o campo só avisa "Enter", então o Shift é lembrado aqui.
            Event::ModifiersChanged(m) =>
                msgs.push(Msg::MacroVisual(macros::EventoVisual::Codigo(macros::EventoCodigo::Modificadores(*m)))),
            Event::Resized(size) => msgs.push(Msg::JanelaRedimensionada(id, *size)),
            // A roda chega em pixels; 40 px valem um passo (como o `ScrollDelta::Pixels` do Iced).
            Event::Wheel { dy, .. } if livre && (dy / 40.0).abs() >= 0.1 => msgs.push(Msg::Roda(dy.signum() as i32)),
            Event::CloseRequested => msgs.push(Msg::Fechar(id)),
            _ => {}
        }
        match msgs.len() {
            0 => None,
            1 => msgs.pop(),
            _ => Some(Msg::Lote(msgs)),
        }
    }

    /// Depois de uma operação grande, o alocador do glibc segura a memória
    /// solta (10 GB de RSS depois do Texto para Colunas, 13/10/2026). Isto
    /// devolve ao sistema e diz no console quanto.
    pub(crate) fn devolver_memoria(operacao: &str) {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            let antes = Self::rss_atual_kb().unwrap_or(0);
            let t = std::time::Instant::now();
            // SAFETY: malloc_trim só percorre as arenas do próprio alocador.
            unsafe { libc::malloc_trim(0); }
            let depois = Self::rss_atual_kb().unwrap_or(0);
            eprintln!("[memória] depois de {operacao}: {:.0} → {:.0} MiB ({:.0} ms)",
                antes as f64 / 1024.0, depois as f64 / 1024.0, t.elapsed().as_secs_f64() * 1000.0);
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        let _ = operacao;
    }

    fn rss_atual_kb() -> Option<u64> {
        std::fs::read_to_string("/proc/self/status").ok()?
            .lines().find(|l| l.starts_with("VmRSS:"))?
            .split_whitespace().nth(1)?.parse().ok()
    }

    pub fn view(&self, id: window::Id) -> Element<'_, Msg> {
        let janela = feather_ui::cromo::moldura_janela(self.view_conteudo(id));
        // A ponta da caneta como clique (a leitura acompanha só a principal).
        let fonte = self.tinta.fonte_pressao.as_ref().filter(|_| self.janelas.tipo(id).is_none() && eh_janela_principal(id));
        feather_ui::clique_caneta::clique_caneta(janela, fonte).into()
    }

    fn view_conteudo(&self, id: window::Id) -> Element<'_, Msg> {
        if let Some(aux) = self.janelas.tipo(id) {
            let body: Element<'_, Msg> = match aux {
                Auxiliar::Configuracoes => self.view_configuracoes(),
                Auxiliar::Ajuda => self.view_ajuda(),
                Auxiliar::Monitor => self.view_monitor(),
                Auxiliar::Macros => self.view_macros(),
                Auxiliar::FormatarCelulas => self.view_formatar_celulas(),
                Auxiliar::DividirTexto => self.view_assistente_divisao(),
                Auxiliar::RemoverDuplicados => self.view_duplicados(),
                Auxiliar::Imprimir => self.view_impressao(),
                Auxiliar::OpenQuery => self.view_openquery(),
                Auxiliar::Inspecao => self.view_inspecao(),
                Auxiliar::PrimeirosPassos => self.view_primeiros_passos(),
                Auxiliar::Novidades => self.view_novidades(),
                Auxiliar::Condicional => self.view_condicional(),
                Auxiliar::Formulario => self.view_formulario_macro(),
            };
            return if self.config.barra_propria {
                cromo::janela_com_barra(&self.titulo(id), body, Self::acoes(id))
            } else { body };
        }
        if self.em_tela_inicio {
            return self.view_tela_inicio(id);
        }
        self.view_editor(id)
    }

}

/// O intervalo como a caixa de nome e os diálogos mostram: `A1`, `A1:B10`,
/// `C:C` ou `5:10`, sem o nome da aba.
pub(crate) fn endereco_area(area: &Rect) -> String {
    feathersheet_core::celulas::refs::format_ref(&Rect::new(area.r0, area.c0, area.r1, area.c1))
}

/// Janela principal, lida pela assinatura de eventos (função sem captura): o
/// movimento do mouse nas janelas auxiliares é descartado antes de virar mensagem,
/// porque cada mensagem faz o Iced reconstruir a tela de todas as janelas.
static JANELA_PRINCIPAL: std::sync::Mutex<Option<window::Id>> = std::sync::Mutex::new(None);

fn definir_janela_principal(id: window::Id) {
    if let Ok(mut j) = JANELA_PRINCIPAL.lock() {
        *j = Some(id);
    }
}

/// Sem janela registrada (testes), vale qualquer uma.
fn eh_janela_principal(id: window::Id) -> bool {
    JANELA_PRINCIPAL.lock().map_or(true, |j| j.is_none_or(|p| p == id))
}

/// A ponte com o laço do pelicano: as janelas, o tema e os eventos.
impl pelicano::App for App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        let tarefa = App::update(self, msg);
        let mut tarefas = vec![tarefa];
        // A formatação condicional mora numa janela própria: o estado manda, a janela acompanha.
        if self.condicional_aberto != self.janelas.aberta(Auxiliar::Condicional) {
            let tam = Self::tamanho_aux(Auxiliar::Condicional);
            tarefas.push(self.janelas.alternar::<Msg>(Auxiliar::Condicional, tam, self.config.barra_propria).map(|_| Msg::AuxAberta));
        }
        // O formulário das macros também: aberto enquanto `formulario_macro` existir.
        if self.formulario_macro.is_some() != self.janelas.aberta(Auxiliar::Formulario) {
            let tam = self.tamanho_formulario();
            tarefas.push(self.janelas.alternar::<Msg>(Auxiliar::Formulario, tam, self.config.barra_propria).map(|_| Msg::AuxAberta));
        }
        if self.condicional_aberto && self.condicional_pegando {
            let endereco = endereco_area(&self.area_selecionada());
            if self.condicional_intervalo != endereco { self.condicional_intervalo = endereco; }
        }
        // O título de cada janela segue o estado (o `title` do daemon do Iced).
        let mut titulos = self.titulos.borrow_mut();
        for id in std::iter::once(WindowId::MAIN).chain(Auxiliar::TODAS.into_iter().filter_map(|a| self.janelas.id(a))) {
            let titulo = self.titulo(id);
            if titulos.get(&id) != Some(&titulo) {
                titulos.insert(id, titulo.clone());
                tarefas.push(Task::window_of(id, pelicano::WindowCmd::Title(titulo)));
            }
        }
        Task::batch(tarefas)
    }

    fn view(&self) -> Element<'_, Msg> {
        App::view(self, WindowId::MAIN)
    }

    fn view_window(&self, id: WindowId) -> Element<'_, Msg> {
        App::view(self, id)
    }

    fn theme(&self) -> pelicano::Theme {
        tema()
    }

    fn subscription(&self) -> Subscription<Msg> {
        App::subscription(self)
    }

    // Chamado antes do `on_window_event`, e só quando nenhum widget usou o evento.
    fn on_unhandled_event(&self, _id: WindowId, _ev: &Event) -> Option<Msg> {
        self.evento_livre.set(true);
        None
    }

    fn on_window_event(&self, id: WindowId, ev: &Event) -> Option<Msg> {
        self.evento_janela(id, ev, self.evento_livre.take())
    }
}
