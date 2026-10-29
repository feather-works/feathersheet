//! Camada de objetos soltos sobre a grade: formas do catálogo comum
//! `feather-shapes` (e, depois, ícones e SmartArt).
//!
//! - Modelo, JSON do campo `ob` e âncora de duas células: `feathersheet_io::objetos_aba`.
//! - Desfazer: `Sheet::alterar_objetos` — cada criar, mover, redimensionar,
//!   girar, ajustar, formatar, colar ou apagar é UM Ctrl+Z.
//! - [`camada`]: o Canvas (desenho, alças, gestos sem tocar no `Sheet`).
//! - [`geometria`]: o ponto de troca com o `feather-shapes`.
//! - [`interface`]: barra flutuante, painéis, menu Formas, menu de contexto
//!   da forma, aba contextual "Editar forma", texto e macro.
//!
//! A barra flutuante, a aba "Editar forma" e o menu de contexto mandam as
//! MESMAS mensagens ([`Evento`]); a lógica fica só aqui.
use super::desenhar::celula_em;
use super::*;
use feathersheet_core::planilha::sheet::Sheet;
use feathersheet_io::objetos_aba::{self as modelo, ObjetoSolto, ObjetosAba, Regua, Retangulo};
use feather_ui::seletor_cor::{self, Evento as EventoCor};
use std::sync::Arc;

mod camada;
pub(in crate::app) use camada::{adiantar_rasteres, esquecer_rasteres};
mod figura;
pub(super) mod geometria;
mod interface;
mod wordart;

pub(super) use camada::arrastando;
use camada::ItemTela;

/// Cores de conteúdo da paleta (as do tema do Office).
pub(super) const PALETA: [&str; 12] = [
    "#4472c4", "#ed7d31", "#a5a5a5", "#ffc000", "#5b9bd5", "#70ad47",
    "#264478", "#9e480e", "#000000", "#ffffff", "#c00000", "#7030a0",
];
pub(super) const ESPESSURAS: [f32; 7] = [0.75, 1.0, 1.5, 2.25, 3.0, 4.5, 6.0];
pub(super) const TRANSPARENCIAS: [f32; 4] = [0.0, 0.25, 0.5, 0.75];
/// Deslocamento de colar/duplicar, em px lógicos.
const PASSO_COLAR: f32 = 16.0;

/// A régua real da aba (larguras/alturas próprias), a mesma da grade.
pub(super) struct ReguaAba<'a>(pub(super) &'a Sheet);

impl Regua for ReguaAba<'_> {
    fn inicio_coluna(&self, c: u32) -> f32 {
        crate::grade::modelo::inicio_x_da(self.0, c)
    }
    fn inicio_linha(&self, r: u32) -> f32 {
        crate::grade::modelo::inicio_y_da(self.0, r)
    }
    fn coluna_em(&self, x: f32) -> u32 {
        celula_em(self.0, x, 0.0).1
    }
    fn linha_em(&self, y: f32) -> u32 {
        celula_em(self.0, 0.0, y).0
    }
}

/// Painel de cores aberto (pela barra ou pela aba).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Painel {
    Preenchimento,
    Contorno,
}

/// O menu Formas abre para inserir ou para trocar a forma selecionada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoMenu {
    Inserir,
    Trocar,
    /// Inserir > Ícones: galeria Tabler com busca (um clique insere).
    Icones,
    /// Inserir > WordArt: estilos prontos (inserem, ou trocam o do WordArt
    /// selecionado) e a deformação do selecionado.
    WordArt,
}

/// Menus ▾ da aba Editar forma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuFaixa {
    Frente,
    Tras,
    Girar,
    Alinhar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ordem {
    Frente,
    Avancar,
    Tras,
    Recuar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Giro {
    Direita90,
    Esquerda90,
    EspelharH,
    EspelharV,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alinhar {
    /// O canto superior esquerdo encosta no canto da célula.
    CantoDaCelula,
    /// Os dois cantos encostam nas bordas das células (ajusta o tamanho).
    AjustarAsCelulas,
}

/// Métricas de um arraste (vão para o Console F12, uma linha por gesto).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Medida {
    pub eventos: u32,
    pub us_eventos: u64,
    pub quadros: u32,
    pub us_desenho: u64,
    pub us_max_desenho: u64,
    pub ms_total: f32,
}

#[derive(Debug, Clone)]
pub enum Evento {
    AlternarMenu(ModoMenu),
    FecharMenu,
    /// Texto da busca da galeria de ícones.
    BuscaIcones(String),
    /// A grade da galeria rolou (deslocamento vertical em px).
    RolarIcones(f32),
    /// Ícone escolhido na galeria: entra perto da célula ativa, selecionado.
    InserirIcone(&'static str),
    /// Estilo pronto escolhido na galeria do WordArt (índice em
    /// `feather_shapes::wordart::ESTILOS`): troca o do WordArt selecionado
    /// ou insere um novo perto da célula ativa.
    EstiloWordArt(usize),
    /// Deformação e intensidade (-1 a 1) do WordArt selecionado.
    DeformarWordArt(feather_shapes::wordart::Transformacao, f32),
    /// Liga/desliga a sombra do WordArt selecionado.
    SombraWordArt(bool),
    /// Forma do catálogo escolhida no menu (inserir ou trocar).
    Escolher(&'static str),
    /// Inserir > Caixa de texto: um retângulo que nasce editando.
    CaixaTexto,
    /// Formatar forma > Efeitos: liga/desliga a sombra da forma selecionada.
    SombraForma,
    Selecionar(u64),
    Desselecionar,
    /// Resultado de mover, redimensionar ou girar (px lógicos absolutos).
    Transformar { id: u64, ret: Retangulo, giro: f32, medida: Medida },
    /// Resultado de arrastar uma alça de ajuste.
    Ajustar { id: u64, forma: &'static str, ajustes: Vec<f32>, medida: Medida },
    /// Arraste de criação terminado.
    Criar { forma: &'static str, ret: Retangulo, espelhar: bool },
    AbrirPainel(Painel),
    FecharPainel,
    /// "Mais cores…" do painel: liga/desliga o seletor completo ao lado.
    MaisCores,
    /// Seletor completo (`feather_ui::seletor_cor`): muda a cor na hora,
    /// como rascunho; vira UM Ctrl+Z ao fechar o painel ou fazer outra coisa.
    Cor(EventoCor),
    Preenchimento(Option<&'static str>),
    Contorno(Option<&'static str>),
    Espessura(f32),
    Transparencia(f32),
    Ordem(Ordem),
    Girar(Giro),
    Alinhar(Alinhar),
    MenuFaixa(MenuFaixa),
    FecharMenuFaixa,
    Recortar,
    Copiar,
    Colar,
    Duplicar,
    Apagar,
    /// Botão direito na forma: seleciona e abre o menu dela.
    Contexto(u64),
    ContextoSub(Option<usize>),
    FecharContexto,
    /// Ação do menu de contexto: fecha o menu e executa.
    DoContexto(Box<Evento>),
    EditarTexto(u64),
    /// Alinhamento do texto na forma (aba Editar forma > Texto).
    AlinharTextoH(modelo::AlinharH),
    AlinharTextoV(modelo::AlinharV),
    TextoDigitado(String),
    TextoConfirmar,
    TextoCancelar,
    AtribuirMacro,
    MacroEscolher(Option<String>),
    MacroConfirmar,
    MacroCancelar,
    ExecutarMacro(u64),
    /// Abre a aba Editar forma.
    FormatarForma,
    CampoLargura(String),
    CampoAltura(String),
    AplicarTamanho,
    TravarProporcao(bool),
    CampoRaio(String),
    CampoCanto(usize, String),
    PorCanto(bool),
    AplicarRaio,
}

/// Cor em escolha pelo seletor completo: aparece na tela, mas só vira
/// passo de desfazer em [`App::confirmar_cor`].
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Rascunho {
    pub id: u64,
    pub painel: Painel,
    /// `#rrggbb`, ou `None` = sem cor.
    pub cor: Option<String>,
    /// A transparência da forma (1 − alfa do seletor).
    pub transparencia: f32,
}

/// Campos numéricos da aba Editar forma (texto como digitado).
#[derive(Debug, Default, Clone)]
pub(super) struct Campos {
    pub largura: String,
    pub altura: String,
    pub raio: String,
    pub cantos: [String; 4],
    pub por_canto: bool,
}

#[derive(Debug, Default)]
pub(super) struct EstadoObjetos {
    /// Aba e id do objeto selecionado.
    pub selecionado: Option<(u32, u64)>,
    /// Forma escolhida no menu, à espera do arraste que a cria.
    pub inserir: Option<&'static str>,
    /// A forma à espera é a Caixa de texto da faixa: nasce com fundo branco
    /// e já editando.
    pub caixa_texto: bool,
    pub menu: Option<ModoMenu>,
    menu_pos: (f32, f32),
    /// Busca digitada na galeria de ícones e a rolagem da grade.
    pub busca_icones: String,
    pub rolagem_icones: f32,
    /// Resultado da busca guardado por termo: a lista inteira é só de
    /// referências; widgets saem apenas para as fileiras à vista.
    lista_icones: std::cell::RefCell<Option<(String, Arc<Vec<&'static feather_shapes::icones::Icone>>)>>,
    pub painel: Option<Painel>,
    painel_pos: (f32, f32),
    /// Seletor completo aberto ao lado do painel, o estado HSVA e o hex digitado.
    pub mais_cores: bool,
    pub cor_hsva: [f32; 4],
    pub cor_hex: String,
    pub rascunho: Option<Rascunho>,
    /// Menu de contexto da forma (posição na janela) e submenu aberto.
    pub contexto: Option<(f32, f32)>,
    pub contexto_sub: Option<usize>,
    pub menu_faixa: Option<(MenuFaixa, f32, f32)>,
    /// Texto em edição: objeto e o texto digitado.
    pub texto: Option<(u64, String)>,
    /// Diálogo Atribuir macro: objeto e a macro escolhida.
    pub macro_dialogo: Option<(u64, Option<String>)>,
    /// Área de transferência interna dos objetos e se o último copiar foi de objeto.
    area: Vec<ObjetoSolto>,
    copia_de_objeto: bool,
    pub travar: bool,
    pub campos: Campos,
    /// Objetos já lidos por aba: (aba, ponteiro e tamanho do texto `ob`).
    cache: std::cell::RefCell<Option<(u32, usize, usize, Arc<ObjetosAba>)>>,
}

/// Lado do ícone inserido, em px lógicos.
const LADO_ICONE: f32 = 64.0;

/// Número com vírgula decimal (pt-BR), sem zeros à toa.
pub(super) fn decimal(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').replace('.', ",")
}

fn numero(t: &str) -> Option<f32> {
    t.trim().replace(',', ".").parse::<f32>().ok().filter(|v| v.is_finite() && *v >= 0.0)
}

impl App {
    /// Objetos da aba ativa (lidos uma vez por versão do texto `ob`).
    fn objetos_da_aba(&self) -> Option<(u32, Arc<ObjetosAba>)> {
        let aba = self.workbook.aba(self.folha)?;
        let Some(texto) = aba.sheet.ob.as_deref() else { return Some((aba.id, Arc::new(ObjetosAba::default()))) };
        let chave = (aba.id, texto.as_ptr() as usize, texto.len());
        if let Some((id, ptr, len, lidos)) = self.objetos.cache.borrow().as_ref() {
            if (*id, *ptr, *len) == chave {
                return Some((aba.id, Arc::clone(lidos)));
            }
        }
        let lidos = Arc::new(ObjetosAba::ler(Some(texto)).unwrap_or_default());
        *self.objetos.cache.borrow_mut() = Some((chave.0, chave.1, chave.2, Arc::clone(&lidos)));
        Some((aba.id, lidos))
    }

    /// O objeto selecionado na aba ativa, se ainda existir (o Ctrl+Z pode tê-lo levado).
    pub(super) fn objeto_selecionado(&self) -> Option<ObjetoSolto> {
        let (aba, id) = self.objetos.selecionado?;
        let (atual, lidos) = self.objetos_da_aba()?;
        (atual == aba).then(|| lidos.obter(id).cloned()).flatten()
    }

    /// A aba contextual "Editar forma" aparece só com forma selecionada.
    pub(super) fn tem_forma_selecionada(&self) -> bool {
        self.objeto_selecionado().is_some()
    }

    fn regua_ret(&self, o: &ObjetoSolto) -> Option<Retangulo> {
        let aba = self.workbook.aba(self.folha)?;
        Some(o.retangulo(&ReguaAba(&aba.sheet)))
    }

    /// Grava os objetos da aba ativa: um passo de desfazer.
    fn gravar_objetos(&mut self, objetos: &ObjetosAba, status: String) -> bool {
        let Some(aba) = self.workbook.aba_mut(self.folha) else { return false };
        if !modelo::gravar_na_aba(&mut aba.sheet, objetos) {
            return false;
        }
        self.alterado = true;
        self.status = status;
        self.sincronizar_campos();
        true
    }

    /// Muda o objeto selecionado com `f` (um Ctrl+Z).
    fn mudar_selecionado(&mut self, status: &str, f: impl FnOnce(&mut ObjetoSolto, &ReguaAba<'_>)) {
        let Some((_, lidos)) = self.objetos_da_aba() else { return };
        let Some((_, id)) = self.objetos.selecionado else { return };
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let mut objetos = (*lidos).clone();
        let Some(o) = objetos.obter_mut(id) else { return };
        f(o, &ReguaAba(&aba.sheet));
        self.gravar_objetos(&objetos, trf!("{status} (Ctrl+Z desfaz)."));
    }

    /// Os campos da aba Editar forma voltam a mostrar o objeto selecionado.
    fn sincronizar_campos(&mut self) {
        let Some(o) = self.objeto_selecionado() else { return };
        let Some(r) = self.regua_ret(&o) else { return };
        let m = r.largura.min(r.altura).max(1.0);
        self.objetos.campos.largura = decimal(r.largura);
        self.objetos.campos.altura = decimal(r.altura);
        if let Some(rs) = geometria::raios(&o.forma, &o.ajustes) {
            self.objetos.campos.raio = decimal(rs[0] * m);
            self.objetos.campos.cantos = rs.map(|v| decimal(v * m));
        }
    }

    fn selecionar(&mut self, id: u64) {
        let aba = self.workbook.aba(self.folha).map(|a| a.id);
        self.objetos.selecionado = aba.map(|a| (a, id));
        self.desselecionar_grafico();
        self.objetos.painel = None;
        self.imagem_selecionada = None;
        self.botao_macro_selecionado = None;
        self.sincronizar_campos();
    }

    /// Tira a seleção; a aba Editar forma some (volta para Inserir).
    fn desselecionar(&mut self) {
        self.objetos.selecionado = None;
        self.desselecionar_grafico();
        self.objetos.painel = None;
        self.objetos.menu_faixa = None;
        if self.faixa_ativa == AbaFaixa::EditarForma {
            self.faixa_ativa = AbaFaixa::Inserir;
        }
    }

    fn registrar_medida(&mut self, gesto: &str, m: Medida) {
        if m.quadros == 0 && m.eventos == 0 {
            return;
        }
        let quadros = m.quadros.max(1) as f32;
        let linha = format!(
            "[objetos] {gesto}: {} eventos de cursor ({:.3} ms cada, sem gravar nem reconstruir a tela), {} quadros em {:.0} ms (intervalo médio {:.1} ms), camada {:.2} ms/quadro (máx {:.2})",
            m.eventos,
            m.us_eventos as f32 / 1000.0 / m.eventos.max(1) as f32,
            m.quadros,
            m.ms_total,
            m.ms_total / quadros,
            m.us_desenho as f32 / 1000.0 / quadros,
            m.us_max_desenho as f32 / 1000.0,
        );
        self.monitor.registrar(feather_ui::monitor::Nivel::Info, tr!("Objetos"), &linha);
    }

    pub(super) fn evento_objetos(&mut self, evento: Evento) -> Task<Msg> {
        match &evento {
            Evento::Cor(_) | Evento::MaisCores => {}
            // A cor da paleta substitui a do rascunho (um passo só).
            Evento::Preenchimento(_) | Evento::Contorno(_) => self.objetos.rascunho = None,
            _ => self.confirmar_cor(),
        }
        match evento {
            Evento::AlternarMenu(modo) => {
                self.objetos.menu = if self.objetos.menu == Some(modo) { None } else { Some(modo) };
                if self.objetos.menu.is_some() {
                    self.objetos.menu_pos = ((self.ponteiro_x - 30.0).max(8.0), self.ponteiro_y + 26.0);
                    self.objetos.painel = None;
                    self.objetos.contexto = None;
                }
                if self.objetos.menu == Some(ModoMenu::Icones) {
                    return Task::focus(interface::ID_BUSCA_ICONES);
                }
            }
            Evento::FecharMenu => self.objetos.menu = None,
            Evento::BuscaIcones(t) => {
                self.objetos.busca_icones = t;
                // Busca nova começa do topo.
                self.objetos.rolagem_icones = 0.0;
                return Task::scroll_to(interface::ID_GRADE_ICONES, pelicano::Point::ORIGIN);
            }
            Evento::RolarIcones(y) => self.objetos.rolagem_icones = y.max(0.0),
            Evento::InserirIcone(nome) => self.inserir_icone(nome),
            Evento::EstiloWordArt(i) => self.estilo_wordart(i),
            Evento::DeformarWordArt(modo, k) => {
                if self.wordart_selecionado().is_some() && k.is_finite() {
                    let k = (k.clamp(-1.0, 1.0) * 10.0).round() / 10.0;
                    self.mudar_selecionado(&trf!("WordArt: {modo} {:.0}%", k * 100.0), |o, _| {
                        o.forma = modo.nome().to_owned();
                        o.ajustes = vec![k];
                    });
                }
            }
            Evento::SombraWordArt(ligada) => {
                if self.wordart_selecionado().is_some() {
                    let status = if ligada { tr!("Sombra do WordArt ligada") } else { tr!("Sombra do WordArt desligada") };
                    self.mudar_selecionado(status, |o, _| wordart::definir_sombra(o, ligada));
                }
            }
            Evento::SombraForma => {
                let ligar = self.objeto_selecionado().is_some_and(|o| o.sombra.is_none());
                let status = if ligar { tr!("Sombra da forma ligada") } else { tr!("Sombra da forma desligada") };
                self.mudar_selecionado(status, |o, _| {
                    o.sombra = ligar.then(|| modelo::Sombra { cor: "#00000059".into(), desfoque: 4.0, dx: 3.0, dy: 3.0 });
                });
            }
            Evento::CaixaTexto => {
                self.objetos.menu = None;
                self.objetos.inserir = Some("retangulo");
                self.objetos.caixa_texto = true;
                self.desselecionar();
                self.status = tr!("Caixa de texto: arraste na planilha para desenhar; um clique cria no tamanho padrão (Esc cancela).").into();
            }
            Evento::Escolher(forma) => {
                self.objetos.caixa_texto = false;
                let modo = self.objetos.menu.take().unwrap_or(ModoMenu::Inserir);
                if modo == ModoMenu::Trocar {
                    let nome = geometria::nome(forma);
                    self.mudar_selecionado(&trf!("Forma trocada por {nome}"), |o, _| {
                        // Ícone ou WordArt trocado por forma vira forma (com a cor de forma).
                        if o.tipo != modelo::TipoObjeto::Forma {
                            o.tipo = modelo::TipoObjeto::Forma;
                            o.aparencia = modelo::Aparencia::padrao_para(forma);
                            o.extras.remove("estilo");
                            o.extras.remove("sombra");
                        }
                        o.forma = forma.to_owned();
                        o.ajustes.clear();
                    });
                } else {
                    self.objetos.inserir = Some(forma);
                    self.desselecionar();
                    self.status = trf!("{}: arraste na planilha para desenhar; um clique cria no tamanho padrão (Shift mantém a proporção, Esc cancela).", geometria::nome(forma));
                }
            }
            Evento::Selecionar(id) => {
                self.selecionar(id);
                if let Some(o) = self.objeto_selecionado() {
                    self.status = trf!("{}: arraste para mover, alças para redimensionar (Delete exclui).", geometria::nome_objeto(&o));
                }
            }
            Evento::Desselecionar => self.desselecionar(),
            Evento::Transformar { id, ret, giro, medida } => {
                let Some(aba) = self.workbook.aba(self.folha) else { return Task::none() };
                let Some((_, lidos)) = self.objetos_da_aba() else { return Task::none() };
                let mut objetos = (*lidos).clone();
                let regua = ReguaAba(&aba.sheet);
                let Some(o) = objetos.obter_mut(id) else { return Task::none() };
                let antes = (o.retangulo(&regua), o.giro);
                o.ancorar(ret, &regua);
                o.definir_giro(giro);
                let gesto = if (antes.1 - o.giro).abs() > 0.01 {
                    "girar"
                } else if (antes.0.largura - ret.largura).abs() > 0.5 || (antes.0.altura - ret.altura).abs() > 0.5 {
                    "redimensionar"
                } else {
                    "mover"
                };
                let status = match gesto { "girar" => tr!("Forma girada"), "redimensionar" => tr!("Forma redimensionada"), _ => tr!("Forma movida") };
                self.gravar_objetos(&objetos, trf!("{status} (Ctrl+Z desfaz)."));
                self.registrar_medida(gesto, medida);
            }
            Evento::Ajustar { id, forma, ajustes, medida } => {
                let Some((_, lidos)) = self.objetos_da_aba() else { return Task::none() };
                let mut objetos = (*lidos).clone();
                let Some(o) = objetos.obter_mut(id) else { return Task::none() };
                o.forma = forma.to_owned();
                o.ajustes = ajustes;
                self.gravar_objetos(&objetos, tr!("Forma ajustada (Ctrl+Z desfaz).").into());
                self.registrar_medida("ajustar", medida);
            }
            Evento::Criar { forma, ret, espelhar } => {
                self.objetos.inserir = None;
                let Some(aba) = self.workbook.aba(self.folha) else { return Task::none() };
                let Some((aba_id, lidos)) = self.objetos_da_aba() else { return Task::none() };
                let mut objetos = (*lidos).clone();
                let mut novo = ObjetoSolto::nova_forma(objetos.proximo_id(), forma, ret, &ReguaAba(&aba.sheet));
                novo.espelhar_h = espelhar;
                if geometria::so_traco(forma) {
                    novo.aparencia = modelo::Aparencia { preenchimento: None, contorno: Some("#4472c4".into()), espessura: 2.0, transparencia: 0.0 };
                }
                let caixa = std::mem::take(&mut self.objetos.caixa_texto);
                if caixa {
                    novo.aparencia = modelo::Aparencia { preenchimento: Some("#ffffff".into()), contorno: Some("#7f7f7f".into()), espessura: 1.0, transparencia: 0.0 };
                    novo.alinhar.h = modelo::AlinharH::Esquerda;
                    novo.alinhar.v = modelo::AlinharV::Topo;
                }
                let Some(id) = objetos.adicionar(novo) else {
                    self.status = trf!("Limite de {} objetos por aba.", modelo::MAX_OBJETOS);
                    return Task::none();
                };
                self.objetos.selecionado = Some((aba_id, id));
                if caixa {
                    self.gravar_objetos(&objetos, tr!("Caixa de texto inserida (Ctrl+Z desfaz).").into());
                    return self.evento_objetos(Evento::EditarTexto(id));
                }
                self.gravar_objetos(&objetos, trf!("{} inserida (Ctrl+Z desfaz).", geometria::nome(forma)));
            }
            Evento::AbrirPainel(p) => {
                self.objetos.painel = (self.objetos.painel != Some(p)).then_some(p);
                self.objetos.mais_cores = false;
                self.objetos.painel_pos = ((self.ponteiro_x - 20.0).max(8.0), self.ponteiro_y + 22.0);
                self.objetos.menu_faixa = None;
            }
            Evento::FecharPainel => {
                self.objetos.painel = None;
                self.objetos.mais_cores = false;
            }
            Evento::MaisCores => {
                self.objetos.mais_cores = !self.objetos.mais_cores;
                if self.objetos.mais_cores {
                    self.preparar_seletor();
                } else {
                    self.confirmar_cor();
                }
            }
            Evento::Cor(e) => self.evento_cor(e),
            Evento::Preenchimento(cor) => {
                self.objetos.painel = None;
                self.mudar_selecionado(tr!("Preenchimento alterado"), |o, _| o.aparencia.preenchimento = cor.map(str::to_owned));
            }
            Evento::Contorno(cor) => {
                self.objetos.painel = None;
                self.mudar_selecionado(tr!("Contorno alterado"), |o, _| o.aparencia.contorno = cor.map(str::to_owned));
            }
            Evento::Espessura(e) => {
                self.objetos.painel = None;
                self.mudar_selecionado(tr!("Espessura do contorno alterada"), |o, _| {
                    o.aparencia.espessura = e;
                    if o.aparencia.contorno.is_none() {
                        o.aparencia.contorno = Some("#2f528f".into());
                    }
                });
            }
            Evento::Transparencia(t) => {
                self.objetos.painel = None;
                self.objetos.mais_cores = false;
                self.mudar_selecionado(tr!("Transparência alterada"), |o, _| o.aparencia.transparencia = t.clamp(0.0, 1.0));
            }
            Evento::Ordem(ordem) => {
                self.objetos.menu_faixa = None;
                let Some((_, lidos)) = self.objetos_da_aba() else { return Task::none() };
                let Some((_, id)) = self.objetos.selecionado else { return Task::none() };
                let mut objetos = (*lidos).clone();
                let lista = &mut objetos.objetos;
                let Some(i) = lista.iter().position(|o| o.id == id) else { return Task::none() };
                let destino = match ordem {
                    Ordem::Frente => lista.len() - 1,
                    Ordem::Avancar => (i + 1).min(lista.len() - 1),
                    Ordem::Tras => 0,
                    Ordem::Recuar => i.saturating_sub(1),
                };
                if destino != i {
                    let o = lista.remove(i);
                    lista.insert(destino, o);
                    let s = match ordem {
                        Ordem::Frente => tr!("Forma trazida para a frente"),
                        Ordem::Avancar => tr!("Forma avançada um nível"),
                        Ordem::Tras => tr!("Forma enviada para trás"),
                        Ordem::Recuar => tr!("Forma recuada um nível"),
                    };
                    self.gravar_objetos(&objetos, trf!("{s} (Ctrl+Z desfaz)."));
                }
            }
            Evento::Girar(g) => {
                self.objetos.menu_faixa = None;
                self.mudar_selecionado(tr!("Forma girada"), |o, _| match g {
                    Giro::Direita90 => o.definir_giro(o.giro + 90.0),
                    Giro::Esquerda90 => o.definir_giro(o.giro - 90.0),
                    Giro::EspelharH => o.espelhar_h = !o.espelhar_h,
                    Giro::EspelharV => o.espelhar_v = !o.espelhar_v,
                });
            }
            Evento::Alinhar(a) => {
                self.objetos.menu_faixa = None;
                self.mudar_selecionado(tr!("Forma alinhada às células"), |o, regua| {
                    let r = o.retangulo(regua);
                    match a {
                        Alinhar::CantoDaCelula => {
                            o.de.dx = 0.0;
                            o.de.dy = 0.0;
                            let (x, y) = o.de.ponto(regua);
                            o.ancorar(Retangulo::novo(x, y, r.largura, r.altura), regua);
                        }
                        Alinhar::AjustarAsCelulas => {
                            o.de.dx = 0.0;
                            o.de.dy = 0.0;
                            if o.ate.dx > 0.0 || o.ate.dy > 0.0 {
                                o.ate.coluna += u32::from(o.ate.dx > 0.0);
                                o.ate.linha += u32::from(o.ate.dy > 0.0);
                            }
                            o.ate.dx = 0.0;
                            o.ate.dy = 0.0;
                        }
                    }
                });
            }
            Evento::MenuFaixa(m) => {
                self.objetos.menu_faixa = match self.objetos.menu_faixa {
                    Some((atual, ..)) if atual == m => None,
                    _ => Some((m, (self.ponteiro_x - 20.0).max(8.0), self.ponteiro_y + 22.0)),
                };
                self.objetos.painel = None;
            }
            Evento::FecharMenuFaixa => self.objetos.menu_faixa = None,
            Evento::Copiar => {
                if let Some(o) = self.objeto_selecionado() {
                    self.objetos.area = vec![o];
                    self.objetos.copia_de_objeto = true;
                    self.status = tr!("Forma copiada (Ctrl+V cola).").into();
                }
            }
            Evento::Recortar => {
                if let Some(o) = self.objeto_selecionado() {
                    self.objetos.area = vec![o];
                    self.objetos.copia_de_objeto = true;
                    self.apagar_objeto_selecionado();
                    self.status = tr!("Forma recortada (Ctrl+V cola, Ctrl+Z desfaz).").into();
                }
            }
            Evento::Colar => {
                let area = self.objetos.area.clone();
                self.colar_objetos(area, "colada");
            }
            Evento::Duplicar => {
                if let Some(o) = self.objeto_selecionado() {
                    self.colar_objetos(vec![o], "duplicada");
                }
            }
            Evento::Apagar => {
                self.apagar_objeto_selecionado();
            }
            Evento::Contexto(id) => {
                if self.objetos.selecionado.map(|s| s.1) != Some(id) {
                    self.selecionar(id);
                }
                self.objetos.contexto = Some((self.ponteiro_x, self.ponteiro_y));
                self.objetos.contexto_sub = None;
                self.objetos.menu = None;
                self.objetos.painel = None;
                self.contexto = None;
            }
            Evento::ContextoSub(s) => self.objetos.contexto_sub = s,
            Evento::FecharContexto => {
                self.objetos.contexto = None;
                self.objetos.contexto_sub = None;
            }
            Evento::DoContexto(e) => {
                self.objetos.contexto = None;
                self.objetos.contexto_sub = None;
                return self.evento_objetos(*e);
            }
            Evento::EditarTexto(id) => {
                self.selecionar(id);
                let texto = self.objeto_selecionado().and_then(|o| o.texto).unwrap_or_default();
                self.objetos.texto = Some((id, texto));
                return Task::focus(interface::ID_TEXTO);
            }
            Evento::AlinharTextoH(h) => self.mudar_selecionado(tr!("Alinhamento do texto alterado"), |o, _| o.alinhar.h = h),
            Evento::AlinharTextoV(v) => self.mudar_selecionado(tr!("Alinhamento do texto alterado"), |o, _| o.alinhar.v = v),
            Evento::TextoDigitado(t) => {
                if let Some((_, atual)) = &mut self.objetos.texto {
                    *atual = t;
                }
            }
            Evento::TextoConfirmar => {
                if let Some((id, t)) = self.objetos.texto.take() {
                    if self.objetos.selecionado.map(|s| s.1) == Some(id) {
                        let t = t.trim().to_owned();
                        self.mudar_selecionado(tr!("Texto da forma alterado"), |o, _| o.texto = (!t.is_empty()).then_some(t));
                    }
                }
            }
            Evento::TextoCancelar => self.objetos.texto = None,
            Evento::AtribuirMacro => {
                if let Some(o) = self.objeto_selecionado() {
                    self.objetos.macro_dialogo = Some((o.id, o.macro_nome.clone()));
                }
            }
            Evento::MacroEscolher(m) => {
                if let Some((_, escolhida)) = &mut self.objetos.macro_dialogo {
                    *escolhida = m;
                }
            }
            Evento::MacroConfirmar => {
                if let Some((id, m)) = self.objetos.macro_dialogo.take() {
                    if self.objetos.selecionado.map(|s| s.1) == Some(id) {
                        let status = match &m {
                            Some(n) => trf!("Macro {n} atribuída à forma: o clique a executa, Ctrl+clique seleciona"),
                            None => tr!("Macro removida da forma").to_owned(),
                        };
                        self.mudar_selecionado(&status, |o, _| o.macro_nome = m);
                    }
                }
            }
            Evento::MacroCancelar => self.objetos.macro_dialogo = None,
            Evento::ExecutarMacro(id) => self.executar_macro_da_forma(id),
            Evento::FormatarForma => {
                if self.tem_forma_selecionada() {
                    self.faixa_ativa = AbaFaixa::EditarForma;
                    self.config.faixa_recolhida = false;
                }
            }
            Evento::CampoLargura(t) => {
                if self.objetos.travar {
                    if let (Some(o), Some(nova)) = (self.objeto_selecionado(), numero(&t)) {
                        if let Some(r) = self.regua_ret(&o).filter(|r| r.largura > 0.0) {
                            self.objetos.campos.altura = decimal(nova * r.altura / r.largura);
                        }
                    }
                }
                self.objetos.campos.largura = t;
            }
            Evento::CampoAltura(t) => {
                if self.objetos.travar {
                    if let (Some(o), Some(nova)) = (self.objeto_selecionado(), numero(&t)) {
                        if let Some(r) = self.regua_ret(&o).filter(|r| r.altura > 0.0) {
                            self.objetos.campos.largura = decimal(nova * r.largura / r.altura);
                        }
                    }
                }
                self.objetos.campos.altura = t;
            }
            Evento::AplicarTamanho => {
                let (Some(w), Some(h)) = (numero(&self.objetos.campos.largura), numero(&self.objetos.campos.altura)) else {
                    self.status = tr!("Tamanho inválido: use números em pixels (ex.: 120 ou 80,5).").into();
                    return Task::none();
                };
                let (w, h) = (w.clamp(modelo::LADO_MINIMO, 8000.0), h.clamp(0.0, 8000.0));
                self.mudar_selecionado(tr!("Tamanho da forma alterado"), |o, regua| {
                    let r = o.retangulo(regua);
                    o.ancorar(Retangulo::novo(r.x, r.y, w, h), regua);
                });
            }
            Evento::TravarProporcao(v) => self.objetos.travar = v,
            Evento::CampoRaio(t) => self.objetos.campos.raio = t,
            Evento::CampoCanto(i, t) => {
                if let Some(c) = self.objetos.campos.cantos.get_mut(i) {
                    *c = t;
                }
            }
            Evento::PorCanto(v) => self.objetos.campos.por_canto = v,
            Evento::AplicarRaio => self.aplicar_raio(),
        }
        Task::none()
    }

    /// Os ícones da galeria para a busca atual. Sem texto: os mais usados
    /// primeiro e depois todos os outros, em ordem alfabética (rolagem
    /// infinita); com texto: todos os que casam, do melhor para o pior.
    pub(super) fn icones_da_busca(&self) -> Arc<Vec<&'static feather_shapes::icones::Icone>> {
        let busca = self.objetos.busca_icones.trim();
        if let Some((termo, lista)) = self.objetos.lista_icones.borrow().as_ref() {
            if termo == busca {
                return lista.clone();
            }
        }
        let todos = feather_shapes::icones::todos();
        let lista: Vec<_> = if busca.is_empty() {
            let populares = feather_shapes::icones::buscar("", usize::MAX);
            let ja: std::collections::HashSet<&str> = populares.iter().map(|i| i.id).collect();
            populares.into_iter().chain(todos.iter().filter(|i| !ja.contains(i.id))).collect()
        } else {
            feather_shapes::icones::buscar(busca, usize::MAX)
        };
        let lista = Arc::new(lista);
        *self.objetos.lista_icones.borrow_mut() = Some((busca.to_owned(), lista.clone()));
        lista
    }

    /// Ícone novo de 64 px perto da célula ativa (como o Excel, que põe o
    /// ícone à vista sem pedir arraste): um Ctrl+Z, já selecionado.
    fn inserir_icone(&mut self, nome: &'static str) {
        self.objetos.menu = None;
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let Some((aba_id, lidos)) = self.objetos_da_aba() else { return };
        let regua = ReguaAba(&aba.sheet);
        let (x, y) = (regua.inicio_coluna(self.coluna) + 8.0, regua.inicio_linha(self.linha) + 8.0);
        let mut objetos = (*lidos).clone();
        let mut novo = ObjetoSolto::nova_forma(objetos.proximo_id(), nome, Retangulo::novo(x, y, LADO_ICONE, LADO_ICONE), &regua);
        novo.tipo = modelo::TipoObjeto::Icone;
        // Traço só: a cor do ícone é o contorno; espessura 1 = traço do Tabler.
        novo.aparencia = modelo::Aparencia { preenchimento: None, contorno: Some("#4472c4".into()), espessura: 1.0, transparencia: 0.0 };
        let Some(id) = objetos.adicionar(novo) else {
            self.status = trf!("Limite de {} objetos por aba.", modelo::MAX_OBJETOS);
            return;
        };
        self.objetos.selecionado = Some((aba_id, id));
        self.gravar_objetos(&objetos, trf!("Ícone {} inserido (Ctrl+Z desfaz).", nome.replace('-', " ")));
    }

    /// O objeto selecionado, se for WordArt.
    pub(super) fn wordart_selecionado(&self) -> Option<ObjetoSolto> {
        self.objeto_selecionado().filter(|o| o.tipo == modelo::TipoObjeto::Wordart)
    }

    /// Galeria do WordArt: com WordArt selecionado troca o estilo dele (um
    /// Ctrl+Z); sem, insere "Seu texto" perto da célula ativa (um Ctrl+Z),
    /// selecionado, como o Excel.
    fn estilo_wordart(&mut self, indice: usize) {
        let estilo = feather_shapes::wordart::estilo(indice);
        if self.wordart_selecionado().is_some() {
            self.mudar_selecionado(&trf!("Estilo do WordArt: {}", estilo.nome), |o, _| wordart::aplicar_estilo(o, estilo));
            return;
        }
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let Some((aba_id, lidos)) = self.objetos_da_aba() else { return };
        let regua = ReguaAba(&aba.sheet);
        let (x, y) = (regua.inicio_coluna(self.coluna) + 8.0, regua.inicio_linha(self.linha) + 8.0);
        let (w, h) = wordart::TAMANHO_NOVO;
        let mut objetos = (*lidos).clone();
        let modo = feather_shapes::wordart::Transformacao::Reto;
        let mut novo = ObjetoSolto::nova_forma(objetos.proximo_id(), modo.nome(), Retangulo::novo(x, y, w, h), &regua);
        novo.tipo = modelo::TipoObjeto::Wordart;
        novo.ajustes = vec![wordart::INTENSIDADE_PADRAO];
        novo.texto = Some(tr!(wordart::TEXTO_PADRAO).to_owned());
        wordart::aplicar_estilo(&mut novo, estilo);
        let Some(id) = objetos.adicionar(novo) else {
            self.status = trf!("Limite de {} objetos por aba.", modelo::MAX_OBJETOS);
            return;
        };
        self.objetos.selecionado = Some((aba_id, id));
        self.gravar_objetos(&objetos, trf!("WordArt {} inserido: F2 ou duplo clique edita o texto (Ctrl+Z desfaz).", estilo.nome));
    }

    /// A cor que o painel aberto mostra: a do rascunho, senão a da forma.
    pub(super) fn cor_do_painel(&self, painel: Painel, o: &ObjetoSolto) -> (Option<String>, f32) {
        if let Some(r) = self.objetos.rascunho.as_ref().filter(|r| r.id == o.id && r.painel == painel) {
            return (r.cor.clone(), r.transparencia);
        }
        let cor = match painel {
            Painel::Preenchimento => o.aparencia.preenchimento.clone(),
            Painel::Contorno => o.aparencia.contorno.clone(),
        };
        (cor, o.aparencia.transparencia)
    }

    /// O seletor começa na cor do painel; o alfa é 1 − transparência da forma.
    fn preparar_seletor(&mut self) {
        let (Some(p), Some(o)) = (self.objetos.painel, self.objeto_selecionado()) else { return };
        let (cor, transparencia) = self.cor_do_painel(p, &o);
        let [r, g, b, _] = cor.as_deref().map_or([0, 0, 0, 255], seletor_cor::rgba_de);
        let alfa = ((1.0 - transparencia).clamp(0.0, 1.0) * 255.0).round() as u8;
        self.objetos.cor_hsva = seletor_cor::rgba_para_hsva([r, g, b, alfa], self.objetos.cor_hsva[0]);
        self.objetos.cor_hex = cor.unwrap_or_else(|| "#000000".into());
    }

    fn hex_do_seletor_objeto(&self) -> String {
        let [h, s, v, a] = self.objetos.cor_hsva;
        let [r, g, b] = seletor_cor::hsv_para_rgb(h, s, v);
        seletor_cor::hex_de([r, g, b, (a * 255.0).round() as u8])
    }

    /// Um evento do seletor completo: atualiza o rascunho (sem desfazer).
    fn evento_cor(&mut self, e: EventoCor) {
        let (Some(painel), Some(o)) = (self.objetos.painel, self.objeto_selecionado()) else { return };
        let transparencia_atual = self.cor_do_painel(painel, &o).1;
        let hsva = &mut self.objetos.cor_hsva;
        let hex: Option<String> = match e {
            EventoCor::Fechar => {
                self.objetos.mais_cores = false;
                self.confirmar_cor();
                return;
            }
            EventoCor::Escolhida(h) if h.is_empty() => None,
            EventoCor::Escolhida(h) => {
                *hsva = seletor_cor::rgba_para_hsva(seletor_cor::rgba_de(&h), hsva[0]);
                Some(h)
            }
            EventoCor::SaturacaoBrilho(s, v) => {
                hsva[1] = s.clamp(0.0, 1.0);
                hsva[2] = v.clamp(0.0, 1.0);
                Some(self.hex_do_seletor_objeto())
            }
            EventoCor::Matiz(h) => {
                hsva[0] = h.rem_euclid(360.0);
                Some(self.hex_do_seletor_objeto())
            }
            EventoCor::Alfa(a) => {
                hsva[3] = a.clamp(0.0, 1.0);
                Some(self.hex_do_seletor_objeto())
            }
            EventoCor::TextoHex(v) => {
                self.objetos.cor_hex = v.clone();
                if !seletor_cor::hex_valido(&v) {
                    return;
                }
                let h = format!("#{}", v.trim().trim_start_matches('#').to_lowercase());
                self.objetos.cor_hsva = seletor_cor::rgba_para_hsva(seletor_cor::rgba_de(&h), self.objetos.cor_hsva[0]);
                self.aplicar_rascunho(painel, o.id, Some(h), transparencia_atual, false);
                return;
            }
            EventoCor::Canal(i, v) => {
                let Ok(n) = v.trim().parse::<u16>() else { return };
                if i > 3 || n > 255 {
                    return;
                }
                let mut canais = seletor_cor::rgba_de(&self.hex_do_seletor_objeto());
                canais[i] = n as u8;
                self.objetos.cor_hsva = seletor_cor::rgba_para_hsva(canais, self.objetos.cor_hsva[0]);
                Some(seletor_cor::hex_de(canais))
            }
        };
        self.aplicar_rascunho(painel, o.id, hex, transparencia_atual, true);
    }

    /// Separa o alfa (vira transparência da forma) e guarda o rascunho.
    fn aplicar_rascunho(&mut self, painel: Painel, id: u64, hex: Option<String>, transparencia_atual: f32, escrever_hex: bool) {
        let (cor, transparencia) = match hex.as_deref() {
            None => (None, transparencia_atual),
            Some(h) => {
                let [r, g, b, a] = seletor_cor::rgba_de(h);
                (Some(format!("#{r:02x}{g:02x}{b:02x}")), 1.0 - f32::from(a) / 255.0)
            }
        };
        if escrever_hex {
            self.objetos.cor_hex = cor.clone().unwrap_or_default();
        }
        self.objetos.rascunho = Some(Rascunho { id, painel, cor, transparencia: (transparencia * 100.0).round() / 100.0 });
    }

    /// Grava o rascunho do seletor: UM Ctrl+Z para toda a escolha.
    pub(super) fn confirmar_cor(&mut self) {
        let Some(r) = self.objetos.rascunho.take() else { return };
        if self.objetos.selecionado.map(|s| s.1) != Some(r.id) {
            return;
        }
        let status = match r.painel {
            Painel::Preenchimento => tr!("Preenchimento alterado"),
            Painel::Contorno => tr!("Contorno alterado"),
        };
        self.mudar_selecionado(status, |o, _| {
            match r.painel {
                Painel::Preenchimento => o.aparencia.preenchimento = r.cor,
                Painel::Contorno => o.aparencia.contorno = r.cor,
            }
            o.aparencia.transparencia = r.transparencia.clamp(0.0, 1.0);
        });
    }

    /// "Arredondar cantos" da barra/aba: raio em px (um para todos ou por canto).
    fn aplicar_raio(&mut self) {
        let Some(o) = self.objeto_selecionado() else { return };
        let Some(r) = self.regua_ret(&o) else { return };
        if geometria::raios(&o.forma, &o.ajustes).is_none() {
            self.status = tr!("Só retângulos têm cantos arredondáveis.").into();
            return;
        }
        let m = r.largura.min(r.altura).max(1.0);
        let c = &self.objetos.campos;
        let fr = |t: &str| numero(t).map(|px| px / m);
        let raios = if c.por_canto {
            let v: Vec<Option<f32>> = c.cantos.iter().map(|t| fr(t)).collect();
            if v.iter().any(Option::is_none) {
                self.status = tr!("Raio inválido: use números em pixels.").into();
                return;
            }
            [v[0].unwrap(), v[1].unwrap(), v[2].unwrap(), v[3].unwrap()]
        } else {
            let Some(f) = fr(&c.raio) else {
                self.status = tr!("Raio inválido: use números em pixels.").into();
                return;
            };
            [f; 4]
        };
        let (id, ajustes) = geometria::com_raios(&o.forma, raios);
        self.mudar_selecionado(tr!("Cantos arredondados"), |o, _| {
            o.forma = id.to_owned();
            o.ajustes = ajustes;
        });
    }

    /// Cola cópias (ids novos, 16 px abaixo e à direita) e seleciona a última.
    fn colar_objetos(&mut self, origem: Vec<ObjetoSolto>, verbo: &str) {
        if origem.is_empty() {
            return;
        }
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let Some((aba_id, lidos)) = self.objetos_da_aba() else { return };
        let regua = ReguaAba(&aba.sheet);
        let mut objetos = (*lidos).clone();
        let mut ultimo = None;
        for o in origem {
            let r = o.retangulo(&regua);
            let mut novo = o.clone();
            novo.id = objetos.proximo_id();
            novo.ancorar(Retangulo::novo(r.x + PASSO_COLAR, r.y + PASSO_COLAR, r.largura, r.altura), &regua);
            ultimo = objetos.adicionar(novo).or(ultimo);
        }
        let Some(id) = ultimo else {
            self.status = trf!("Limite de {} objetos por aba.", modelo::MAX_OBJETOS);
            return;
        };
        self.objetos.selecionado = Some((aba_id, id));
        self.gravar_objetos(&objetos, trf!("Forma {verbo} (Ctrl+Z desfaz)."));
        // A próxima colagem anda mais um passo, como no Excel.
        if verbo == "colada" {
            if let Some(o) = self.objeto_selecionado() {
                self.objetos.area = vec![o];
            }
        }
    }

    /// Clique na forma com macro: o mesmo caminho dos botões de macro
    /// (histórico do Alt+F11, erros com linha/coluna, escritas revertidas).
    fn executar_macro_da_forma(&mut self, id: u64) {
        if !self.confirmar() || !self.macros_liberadas() {
            return;
        }
        let Some((_, lidos)) = self.objetos_da_aba() else { return };
        let Some(o) = lidos.obter(id) else { return };
        let Some(nome) = o.macro_nome.clone() else { return };
        let rotulo = trf!("forma {}", geometria::nome(&o.forma));
        let Some((nome, codigo)) = feathersheet_macros::procedimentos::codigo_da_macro(&self.macro_catalogo, &nome) else {
            self.status = trf!("Macro \"{nome}\" não encontrada na biblioteca.");
            return;
        };
        self.rodar_macro_em_fundo(historico_macros::GatilhoMacro::Botao { rotulo }, &nome, &codigo,
            macro_dialogo::em_fundo::DepoisDaMacro::Forma);
    }

    /// Delete com objeto selecionado: exclui (um Ctrl+Z). Devolve se havia.
    pub(super) fn apagar_objeto_selecionado(&mut self) -> bool {
        if self.objeto_selecionado().is_none() {
            self.objetos.selecionado = None;
            return false;
        }
        let Some((_, lidos)) = self.objetos_da_aba() else { return false };
        let Some((_, id)) = self.objetos.selecionado else { return false };
        let mut objetos = (*lidos).clone();
        let nome_forma = objetos.remover(id).map(|o| geometria::nome(&o.forma)).unwrap_or_default();
        self.desselecionar();
        self.gravar_objetos(&objetos, trf!("{nome_forma} excluída (Ctrl+Z desfaz)."))
    }

    /// Setas com objeto selecionado: move 1 px lógico (um Ctrl+Z por toque).
    pub(super) fn mover_objeto_teclado(&mut self, dx: f32, dy: f32) -> bool {
        let Some(o) = self.objeto_selecionado() else { return false };
        let Some(r) = self.regua_ret(&o) else { return false };
        let ret = camada::mover(r, (dx, dy));
        let _ = self.evento_objetos(Evento::Transformar { id: o.id, ret, giro: o.giro, medida: Medida::default() });
        true
    }

    /// Atalhos com forma selecionada (Delete, setas, Ctrl+C/X/V/D). Devolve
    /// se a tecla foi usada (então não vai para as células).
    pub(super) fn teclado_objetos(&mut self, key: &keyboard::Key, mods: keyboard::Modifiers) -> bool {
        use keyboard::key::Named as N;
        if self.objetos.texto.is_some() {
            return false;
        }
        let selecionado = self.tem_forma_selecionada();
        if mods.control() {
            let letra = match key {
                keyboard::Key::Character(c) => c.to_lowercase(),
                _ => return false,
            };
            if !selecionado && self.atalho_grafico(&letra) {
                return true;
            }
            if selecionado && letra == "c" {
                self.graficos_esquecer_copia();
            }
            let evento = match (letra.as_str(), selecionado) {
                ("c", true) => Evento::Copiar,
                ("x", true) => Evento::Recortar,
                ("d", true) => Evento::Duplicar,
                ("v", _) if self.objetos.copia_de_objeto && !self.objetos.area.is_empty() => Evento::Colar,
                // Copiar células depois de copiar uma forma: Ctrl+V volta a colar células.
                ("c" | "x", false) => {
                    self.objetos.copia_de_objeto = false;
                    return false;
                }
                _ => return false,
            };
            let _ = self.evento_objetos(evento);
            return true;
        }
        if !selecionado {
            // Gráfico selecionado: Delete exclui (como no Excel).
            if !mods.control() && matches!(key, keyboard::Key::Named(N::Delete | N::Backspace)) {
                return self.apagar_grafico_selecionado();
            }
            return false;
        }
        match key {
            keyboard::Key::Named(N::Delete | N::Backspace) => self.apagar_objeto_selecionado(),
            keyboard::Key::Named(N::ArrowLeft) => self.mover_objeto_teclado(-1.0, 0.0),
            keyboard::Key::Named(N::ArrowRight) => self.mover_objeto_teclado(1.0, 0.0),
            keyboard::Key::Named(N::ArrowUp) => self.mover_objeto_teclado(0.0, -1.0),
            keyboard::Key::Named(N::ArrowDown) => self.mover_objeto_teclado(0.0, 1.0),
            keyboard::Key::Named(N::F2) => {
                if let Some(o) = self.objeto_selecionado() {
                    let _ = self.evento_objetos(Evento::EditarTexto(o.id));
                }
                true
            }
            _ => false,
        }
    }

    /// Esc: fecha diálogo/menu/painel/texto, cancela a inserção ou tira a seleção.
    pub(super) fn esc_objetos(&mut self) -> bool {
        // Um gesto que perdeu o soltar do mouse não pode prender o cursor global.
        camada::soltar_arraste();
        self.confirmar_cor();
        self.objetos.mais_cores = false;
        let o = &mut self.objetos;
        if o.macro_dialogo.take().is_some()
            || o.texto.take().is_some()
            || o.contexto.take().is_some()
            || o.menu.take().is_some()
            || o.menu_faixa.take().is_some()
            || o.painel.take().is_some()
        {
            return true;
        }
        if o.inserir.take().is_some() {
            self.status = tr!("Inserção de forma cancelada.").into();
            return true;
        }
        if o.selecionado.is_some() {
            self.desselecionar();
            return true;
        }
        false
    }

    /// `FEATHERSHEET_CAPTURAR_OBJETOS=1` (ou `menu`, `painel`, `contexto`,
    /// `faixa`, `icones` = galeria buscando "seta", `icone` = quatro ícones
    /// inseridos, `wordart` = um WordArt por estilo e a galeria aberta): aba Inserir, formas de exemplo com aparência variada e a
    /// estrela selecionada; os modos abrem o menu Formas, o painel de
    /// contorno, o menu de contexto ou a aba Editar forma.
    pub(super) fn preparar_captura_objetos(&mut self, modo: &str) {
        self.faixa_ativa = AbaFaixa::Inserir;
        let lugares: [(&'static str, (f32, f32, f32, f32)); 8] = [
            ("retangulo", (40.0, 40.0, 160.0, 90.0)),
            ("retangulo-arredondado", (240.0, 40.0, 160.0, 90.0)),
            ("elipse", (440.0, 40.0, 140.0, 90.0)),
            ("triangulo", (620.0, 30.0, 110.0, 100.0)),
            ("losango", (40.0, 190.0, 120.0, 110.0)),
            ("seta-direita", (220.0, 200.0, 200.0, 90.0)),
            ("linha", (460.0, 190.0, 160.0, 110.0)),
            ("estrela-5", (680.0, 190.0, 130.0, 125.0)),
        ];
        for (forma, (x, y, w, h)) in lugares {
            let _ = self.evento_objetos(Evento::Escolher(forma));
            let _ = self.evento_objetos(Evento::Criar { forma, ret: Retangulo::novo(x, y, w, h), espelhar: false });
        }
        if let Some((_, lidos)) = self.objetos_da_aba() {
            let mut objetos = (*lidos).clone();
            for o in &mut objetos.objetos {
                match o.forma.as_str() {
                    "retangulo" => o.ajustes = vec![0.18, 0.18, 0.0, 0.0],
                    "retangulo-arredondado" => {
                        o.aparencia.preenchimento = Some("#70ad47".into());
                        o.aparencia.contorno = Some("#3c6e24".into());
                        o.texto = Some(tr!("Etapa 1").into());
                        o.macro_nome = Some("Relatorio".into());
                    }
                    "elipse" => {
                        o.aparencia.preenchimento = Some("#ffc000".into());
                        o.aparencia.transparencia = 0.25;
                    }
                    "triangulo" => {
                        o.aparencia.preenchimento = None;
                        o.aparencia.contorno = Some("#c00000".into());
                        o.aparencia.espessura = 3.0;
                        o.ajustes = vec![0.25];
                    }
                    "seta-direita" => {
                        o.aparencia.preenchimento = Some("#ed7d31".into());
                        o.definir_giro(-12.0);
                    }
                    "estrela-5" => o.definir_giro(10.0),
                    _ => {}
                }
            }
            self.gravar_objetos(&objetos, String::new());
            if let Some(estrela) = objetos.objetos.iter().find(|o| o.forma == "estrela-5") {
                let _ = self.evento_objetos(Evento::Selecionar(estrela.id));
            }
        }
        self.ponteiro_x = 300.0;
        self.ponteiro_y = 70.0;
        match modo {
            "menu" => {
                let _ = self.evento_objetos(Evento::AlternarMenu(ModoMenu::Inserir));
            }
            "icones" => {
                self.ponteiro_x = 560.0;
                let _ = self.evento_objetos(Evento::AlternarMenu(ModoMenu::Icones));
                let _ = self.evento_objetos(Evento::BuscaIcones("seta".into()));
            }
            "wordart" => {
                // Quatro WordArts (um por estilo) abaixo das formas, com deformações
                // diferentes; o último selecionado e a galeria aberta.
                let modos = [("reto", 0.5, tr!("Vendas 2026")), ("arco", 0.6, tr!("Relatório")), ("onda", 0.7, tr!("Metas")), ("inclinado", 0.5, tr!("Ótimo!"))];
                for (i, (modo, k, texto)) in modos.into_iter().enumerate() {
                    self.objetos.selecionado = None;
                    self.linha = [12, 12, 15, 15][i];
                    self.coluna = [0, 4, 0, 4][i];
                    let _ = self.evento_objetos(Evento::EstiloWordArt(i));
                    self.mudar_selecionado("", |o, _| {
                        o.forma = modo.to_owned();
                        o.ajustes = vec![k];
                        o.texto = Some(texto.to_owned());
                    });
                }
                self.linha = 0;
                self.coluna = 0;
                self.ponteiro_x = 620.0;
                self.ponteiro_y = 70.0;
                let _ = self.evento_objetos(Evento::AlternarMenu(ModoMenu::WordArt));
            }
            "icone" => {
                // Ícones inseridos abaixo das formas; o último fica selecionado e girado.
                for (coluna, nome) in [(1, "home"), (2, "chart-bar"), (3, "heart"), (4, "settings")] {
                    self.linha = 13;
                    self.coluna = coluna;
                    let _ = self.evento_objetos(Evento::InserirIcone(nome));
                }
                self.linha = 0;
                self.coluna = 0;
            }
            "painel" => {
                self.ponteiro_x = 760.0;
                self.ponteiro_y = 300.0;
                let _ = self.evento_objetos(Evento::AbrirPainel(Painel::Contorno));
            }
            "cores" => {
                self.ponteiro_x = 560.0;
                self.ponteiro_y = 200.0;
                let _ = self.evento_objetos(Evento::AbrirPainel(Painel::Preenchimento));
                let _ = self.evento_objetos(Evento::MaisCores);
                let _ = self.evento_objetos(Evento::Cor(EventoCor::Escolhida("#1a73e8cc".into())));
            }
            "contexto" => {
                self.ponteiro_x = 700.0;
                self.ponteiro_y = 330.0;
                if let Some(o) = self.objeto_selecionado() {
                    let _ = self.evento_objetos(Evento::Contexto(o.id));
                    let _ = self.evento_objetos(Evento::ContextoSub(Some(interface::SUB_FRENTE)));
                }
            }
            "faixa" | "ajuste" => {
                let alvo = if modo == "ajuste" { "retangulo" } else { "estrela-5" };
                if let Some(id) = self.objetos_da_aba().and_then(|(_, l)| l.objetos.iter().find(|o| o.forma == alvo).map(|o| o.id)) {
                    let _ = self.evento_objetos(Evento::Selecionar(id));
                }
                let _ = self.evento_objetos(Evento::FormatarForma);
            }
            "texto" => {
                // Aba Editar forma com o grupo Texto, um retângulo com texto
                // embaixo à direita e o retângulo arredondado editando no lugar.
                let achar = |app: &Self, f: &str| app.objetos_da_aba().and_then(|(_, l)| l.objetos.iter().find(|o| o.forma == f).map(|o| o.id));
                if let Some(id) = achar(self, "retangulo") {
                    let _ = self.evento_objetos(Evento::Selecionar(id));
                    let _ = self.evento_objetos(Evento::EditarTexto(id));
                    let _ = self.evento_objetos(Evento::TextoDigitado(tr!("Canto").into()));
                    let _ = self.evento_objetos(Evento::TextoConfirmar);
                    let _ = self.evento_objetos(Evento::AlinharTextoH(modelo::AlinharH::Direita));
                    let _ = self.evento_objetos(Evento::AlinharTextoV(modelo::AlinharV::Baixo));
                }
                if let Some(id) = achar(self, "retangulo-arredondado") {
                    let _ = self.evento_objetos(Evento::Selecionar(id));
                    let _ = self.evento_objetos(Evento::FormatarForma);
                    let _ = self.evento_objetos(Evento::EditarTexto(id));
                    let _ = self.evento_objetos(Evento::TextoDigitado(tr!("Etapa 1 editando").into()));
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod testes;
