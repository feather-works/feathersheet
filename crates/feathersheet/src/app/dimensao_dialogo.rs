//! Diálogos "Altura da linha" e "Largura da coluna" do menu Formatar
//! (grupo Células da Página Inicial), como no Excel. O valor vale para
//! todas as linhas/colunas da seleção, num único Ctrl+Z. "Largura padrão"
//! usa o mesmo diálogo e vale para as colunas sem largura própria da aba.
use super::*;
use crate::grade::ALTURA_PADRAO;
use feathersheet_core::celulas::refs::col_name;

const ID_CAMPO: &str = "feathersheet.dimensao-valor";

/// Acima disto a seleção não recebe tamanho avulso: cada linha vira uma
/// entrada no mapa de dimensões e um passo de desfazer.
const TETO_EIXOS: u32 = 100_000;
const MINIMO: u32 = 1;
const MAXIMO: u32 = 2_000;

#[derive(Debug, Clone)]
pub enum Evento { Abrir(bool), AbrirPadrao, Valor(String), Aplicar, Fechar }

#[derive(Debug, Clone)]
pub(super) struct Estado {
    /// `true` = largura de coluna; `false` = altura de linha.
    pub coluna: bool,
    pub a: u32,
    pub b: u32,
    pub valor: String,
    pub erro: Option<String>,
    /// Largura padrão da aba, não das colunas da seleção.
    pub padrao: bool,
}

/// Aceita "22", "22,5" e "22.5"; arredonda para pixel inteiro.
fn ler_valor(texto: &str) -> Option<u32> {
    let n: f32 = texto.trim().replace(',', ".").parse().ok()?;
    if !n.is_finite() { return None; }
    let px = n.round();
    (px >= MINIMO as f32 && px <= MAXIMO as f32).then_some(px as u32)
}

fn nome_eixo(coluna: bool, i: u32) -> String {
    if coluna { col_name(i) } else { (i + 1).to_string() }
}

impl App {
    fn abrir_dimensao(&mut self, coluna: bool) {
        if !self.confirmar() { return; }
        let area = self.area_selecionada();
        let (a, b) = if coluna { (area.c0, area.c1) } else { (area.r0, area.r1) };
        if b - a + 1 > TETO_EIXOS {
            self.status = trf!(
                "Selecione até {} {} para definir o tamanho",
                TETO_EIXOS, if coluna { "colunas" } else { "linhas" });
            return;
        }
        // O campo vem com o tamanho atual quando todos são iguais (Excel).
        let valor = self.workbook.aba(self.folha).and_then(|aba| {
            let (mapa, padrao) = if coluna {
                (&aba.sheet.col_widths, crate::grade::largura_padrao_da(&aba.sheet) as u32)
            } else { (&aba.sheet.row_heights, ALTURA_PADRAO as u32) };
            let tamanho = |i: u32| mapa.get(&i).copied().unwrap_or(padrao);
            let primeiro = tamanho(a);
            (a..=b).all(|i| tamanho(i) == primeiro).then(|| primeiro.to_string())
        }).unwrap_or_default();
        self.edicao_menu = None;
        self.contexto = None;
        self.dimensao_modal = Some(Estado { coluna, a, b, valor, erro: None, padrao: false });
    }

    fn abrir_largura_padrao(&mut self) {
        if !self.confirmar() { return; }
        let valor = self.workbook.aba(self.folha)
            .map(|aba| (crate::grade::largura_padrao_da(&aba.sheet) as u32).to_string())
            .unwrap_or_default();
        self.edicao_menu = None;
        self.contexto = None;
        self.dimensao_modal = Some(Estado { coluna: true, a: 0, b: 0, valor, erro: None, padrao: true });
    }

    fn aplicar_dimensao(&mut self) {
        let Some(estado) = self.dimensao_modal.clone() else { return };
        let Some(px) = ler_valor(&estado.valor) else {
            if let Some(modal) = &mut self.dimensao_modal {
                modal.erro = Some(trf!("Digite um número inteiro de {MINIMO} a {MAXIMO} pixels."));
            }
            return;
        };
        if estado.padrao {
            let mut mudou = false;
            self.workbook.editar(self.folha, |sheet| { mudou = sheet.definir_largura_padrao(Some(px)); });
            self.alterado |= mudou;
            self.status = trf!("Largura padrão das colunas: {px} px");
            self.dimensao_modal = None;
            return;
        }
        let mut mudou = false;
        self.workbook.editar(self.folha, |sheet| {
            sheet.abrir_transacao();
            for i in estado.a..=estado.b {
                mudou |= sheet.definir_dimensao(estado.coluna, i, px);
            }
            sheet.fechar_transacao();
        });
        self.alterado |= mudou;
        let quantos = estado.b - estado.a + 1;
        self.status = match (estado.coluna, quantos) {
            (true, 1) => trf!("Largura da coluna {}: {px} px", nome_eixo(true, estado.a)),
            (true, n) => trf!("{n} colunas com largura de {px} px"),
            (false, 1) => trf!("Altura da linha {}: {px} px", nome_eixo(false, estado.a)),
            (false, n) => trf!("{n} linhas com altura de {px} px"),
        };
        self.dimensao_modal = None;
    }

    pub(super) fn evento_dimensao(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir(_) | Evento::AbrirPadrao => {
                match evento {
                    Evento::Abrir(coluna) => self.abrir_dimensao(coluna),
                    _ => self.abrir_largura_padrao(),
                }
                if self.dimensao_modal.is_some() {
                    // shortcut: o pelicano não seleciona o texto do campo por Task; o valor não chega selecionado, até existir `Task::select_all`.
                    return Task::focus(ID_CAMPO);
                }
            }
            Evento::Valor(valor) => if let Some(modal) = &mut self.dimensao_modal {
                modal.valor = valor;
                modal.erro = None;
            },
            Evento::Aplicar => self.aplicar_dimensao(),
            Evento::Fechar => self.dimensao_modal = None,
        }
        Task::none()
    }

    /// Ajusta a altura das linhas da seleção ao conteúdo (par do AutoAjuste
    /// de largura que já existia).
    pub(super) fn ajustar_altura_da_selecao(&mut self) {
        let area = self.area_selecionada();
        self.ajustar_linhas(area.r0, area.r1);
    }

    fn view_dimensao(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let s = self.dimensao_modal.as_ref().expect("diálogo de dimensão aberto");
        let (titulo, rotulo, plural) = if s.padrao {
            (tr!("Largura padrão"), tr!("Largura (px)"), tr!("Colunas"))
        } else if s.coluna {
            (tr!("Largura da coluna"), tr!("Largura (px)"), tr!("Colunas"))
        } else { (tr!("Altura da linha"), tr!("Altura (px)"), tr!("Linhas")) };
        let alvo = if s.padrao {
            tr!("Vale para as colunas desta aba que não têm largura própria").to_string()
        } else if s.a == s.b {
            format!("{} {}", &plural[..plural.len() - 1], nome_eixo(s.coluna, s.a))
        } else {
            trf!("{plural} {} a {}", nome_eixo(s.coluna, s.a), nome_eixo(s.coluna, s.b))
        };
        let campo = text_input(if s.a == s.b { "" } else { tr!("Tamanhos diferentes") }, &s.valor)
            .id(ID_CAMPO)
            .on_input(|v| Msg::Dimensao(Evento::Valor(v)))
            .on_submit(Msg::Dimensao(Evento::Aplicar))
            .padding(9).width(Length::Fixed(120.0));
        let mut painel = pelicano::column![
            row![text(rotulo).size(13), campo].spacing(12).align(pelicano::Align::Center),
        ].spacing(10);
        if let Some(erro) = &s.erro {
            painel = painel.push(row![
                feather_ui::icones::icone("info-circle", 15.0),
                text(erro.clone()).size(12).color(COR_ERRO()),
            ].spacing(6).align(pelicano::Align::Center));
        }
        self.dialogo(titulo, painel)
            .icone(if s.coluna || s.padrao { "arrows-horizontal" } else { "arrows-vertical" })
            .subtitulo(alvo)
            .largura(420.0)
            .ao_fechar(Msg::Dimensao(Evento::Fechar))
            .secundario(tr!("Cancelar"), Some(Msg::Dimensao(Evento::Fechar)))
            .primario("OK", Some(Msg::Dimensao(Evento::Aplicar)))
    }

    pub(super) fn sobrepor_dimensao<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.dimensao_modal.is_none() { return base; }
        self.view_dimensao().sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn largura(app: &App, c: u32) -> Option<u32> {
        app.workbook.aba(0).unwrap().sheet.col_widths.get(&c).copied()
    }

    #[test]
    fn le_numero_com_virgula_e_recusa_fora_da_faixa() {
        assert_eq!(ler_valor("22"), Some(22));
        assert_eq!(ler_valor(" 22,6 "), Some(23));
        assert_eq!(ler_valor("0"), None);
        assert_eq!(ler_valor("9999"), None);
        assert_eq!(ler_valor("abc"), None);
    }

    #[test]
    fn largura_vale_para_todas_as_colunas_e_um_desfazer_volta() {
        let mut app = App::default();
        let _ = app.update(Msg::SelecionarColuna(1));
        let _ = app.update(Msg::EstenderColuna(3));
        let _ = app.update(Msg::Dimensao(Evento::Abrir(true)));
        let modal = app.dimensao_modal.as_ref().expect("abriu");
        assert_eq!((modal.a, modal.b), (1, 3));
        assert_eq!(modal.valor, (crate::grade::LARGURA_PADRAO as u32).to_string());
        let _: Element<'_, Msg> = app.view_dimensao().painel();
        let _ = app.update(Msg::Dimensao(Evento::Valor("80".into())));
        let _ = app.update(Msg::Dimensao(Evento::Aplicar));
        assert!(app.dimensao_modal.is_none());
        assert_eq!((largura(&app, 1), largura(&app, 3)), (Some(80), Some(80)));
        assert_eq!(largura(&app, 4), None);
        let _ = app.update(Msg::Desfazer);
        assert_eq!((largura(&app, 1), largura(&app, 3)), (None, None));
    }

    #[test]
    fn largura_padrao_muda_so_as_colunas_sem_largura_propria() {
        let mut app = App::default();
        app.workbook.editar(0, |s| { s.definir_dimensao(true, 2, 60); });
        let _ = app.update(Msg::Dimensao(Evento::AbrirPadrao));
        assert_eq!(app.dimensao_modal.as_ref().unwrap().valor, (crate::grade::LARGURA_PADRAO as u32).to_string());
        let _: Element<'_, Msg> = app.view_dimensao().painel();
        let _ = app.update(Msg::Dimensao(Evento::Valor("90".into())));
        let _ = app.update(Msg::Dimensao(Evento::Aplicar));
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(sheet.largura_padrao, Some(90));
        assert_eq!(crate::grade::largura_padrao_da(sheet), 90.0);
        assert_eq!(sheet.col_widths.get(&2), Some(&60), "a coluna com largura própria fica");
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.largura_padrao, None);
    }

    #[test]
    fn valor_invalido_mantem_o_dialogo_com_aviso() {
        let mut app = App::default();
        let _ = app.update(Msg::Dimensao(Evento::Abrir(false)));
        let _ = app.update(Msg::Dimensao(Evento::Valor("0".into())));
        let _ = app.update(Msg::Dimensao(Evento::Aplicar));
        assert!(app.dimensao_modal.as_ref().is_some_and(|m| m.erro.is_some()));
        let _ = app.update(Msg::Dimensao(Evento::Fechar));
        assert!(app.dimensao_modal.is_none());
    }

    #[test]
    fn tamanhos_diferentes_deixam_o_campo_vazio() {
        let mut app = App::default();
        app.workbook.editar(0, |s| { s.definir_dimensao(false, 1, 40); });
        let _ = app.update(Msg::Selecionar(0, 0));
        let _ = app.update(Msg::Estender(2, 0));
        let _ = app.update(Msg::Dimensao(Evento::Abrir(false)));
        assert_eq!(app.dimensao_modal.as_ref().unwrap().valor, "");
    }
}
