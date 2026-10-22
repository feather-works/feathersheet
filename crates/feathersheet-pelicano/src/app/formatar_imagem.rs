//! Painel "Formatar imagem" (17/10/2026), ao lado da grade como o Formatar
//! gráfico e no desenho do painel do Excel: Tamanho e Propriedades da imagem
//! solta selecionada. Cada mudança grava no item do bloco `im` da aba (um
//! Ctrl+Z). Medidas em cm a 96 px por polegada, como o Excel.
use super::imagens_importadas::{self, Posicao, Posicionamento};
use super::{App, Msg};
use feather_ui::campo_numero::{campo_numero, escrever, ler, Limites};
use feather_ui::tema::{COR_BORDA, COR_PAINEL_FAIXA, COR_TEXTO, COR_TEXTO_MUTED};
use pelicano::button; use pelicano::checkbox; use pelicano::column; use pelicano::container; use pelicano::radio; use pelicano::row; use pelicano::text; use pelicano::Space;
use pelicano::Element; use pelicano::Length;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;

const LARGURA_PAINEL: f32 = 300.0;
const PX_POR_CM: f64 = 96.0 / 2.54;
const LADO_MAXIMO: f64 = 4096.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Campo {
    Altura,
    Largura,
    Rotacao,
    EscalaAltura,
    EscalaLargura,
}

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Fechar,
    Campo(Campo, String),
    FixarProporcao(bool),
    RelativoOriginal(bool),
    Redefinir,
    Posicionamento(Posicionamento),
    Imprimir(bool),
    Bloqueado(bool),
}

#[derive(Debug, Default)]
pub struct Estado {
    pub aberto: bool,
    /// O que está digitado em cada campo (pode ser "1," no meio da digitação).
    rascunhos: HashMap<Campo, String>,
    /// Tamanho em pixels da figura, por id (decodificar o base64 a cada
    /// quadro custaria caro).
    original: RefCell<Option<(String, Option<(u32, u32)>)>>,
}

/// O que o painel mostra da imagem.
struct Medidas {
    pos: Posicao,
    giro: f64,
    original: Option<(u32, u32)>,
    /// Base das escalas: o tamanho original ou o tamanho ao inserir.
    base: (f64, f64),
    fixar: bool,
    relativo: bool,
    posicionamento: Posicionamento,
    imprimir: bool,
    bloqueado: bool,
}

fn base_da_escala(item: &Value, original: Option<(u32, u32)>, relativo: bool) -> (f64, f64) {
    let original = original.map(|(w, h)| (f64::from(w), f64::from(h)));
    let inserida = item["base"]["width"].as_f64().zip(item["base"]["height"].as_f64());
    let base = if relativo { original.or(inserida) } else { inserida.or(original) };
    base.filter(|(w, h)| *w > 0.0 && *h > 0.0).unwrap_or((1.0, 1.0))
}

impl Medidas {
    fn de(item: &Value, sheet: &feathersheet_core::planilha::sheet::Sheet, original: Option<(u32, u32)>) -> Option<Self> {
        let relativo = item["relativeToOriginal"].as_bool().unwrap_or(true);
        Some(Self {
            pos: Posicao::efetiva(item, sheet)?,
            giro: item["rotation"].as_f64().unwrap_or(0.0),
            original,
            base: base_da_escala(item, original, relativo),
            fixar: item["lockAspect"].as_bool().unwrap_or(true),
            relativo,
            posicionamento: Posicionamento::de(item),
            imprimir: item["print"].as_bool().unwrap_or(true),
            bloqueado: item["locked"].as_bool().unwrap_or(true),
        })
    }

    fn valor(&self, campo: Campo) -> f64 {
        match campo {
            Campo::Altura => self.pos.height / PX_POR_CM,
            Campo::Largura => self.pos.width / PX_POR_CM,
            Campo::Rotacao => self.giro,
            Campo::EscalaAltura => self.pos.height / self.base.1 * 100.0,
            Campo::EscalaLargura => self.pos.width / self.base.0 * 100.0,
        }
    }
}

/// O novo tamanho `(largura, altura)` em px pelo valor digitado num campo.
fn novo_tamanho(m: &Medidas, campo: Campo, v: f64) -> (f64, f64) {
    let (w, h) = (m.pos.width, m.pos.height);
    let razao = w / h;
    let (nw, nh) = match campo {
        Campo::Altura | Campo::EscalaAltura => {
            let nh = if campo == Campo::Altura { v * PX_POR_CM } else { m.base.1 * v / 100.0 };
            (if m.fixar { nh * razao } else { w }, nh)
        }
        _ => {
            let nw = if campo == Campo::Largura { v * PX_POR_CM } else { m.base.0 * v / 100.0 };
            (nw, if m.fixar { nw / razao } else { h })
        }
    };
    (nw.round().clamp(1.0, LADO_MAXIMO), nh.round().clamp(1.0, LADO_MAXIMO))
}

impl App {
    /// O item e as medidas da imagem solta selecionada.
    fn imagem_para_formatar(&self) -> Option<(String, Medidas)> {
        let (aba_id, id) = self.imagem_selecionada.as_ref()?;
        let aba = self.workbook.aba(self.folha).filter(|a| a.id == *aba_id)?;
        let lista = self.itens_cached(aba.id, &aba.sheet).ok()?;
        let item = lista.iter().find(|v| v["id"].as_str() == Some(id.as_str()))?;
        let original = {
            let mut cache = self.formatar_imagem.original.borrow_mut();
            match cache.as_ref() {
                Some((guardado, o)) if guardado == id => *o,
                _ => {
                    let o = imagens_importadas::tamanho_original(item);
                    *cache = Some((id.clone(), o));
                    o
                }
            }
        };
        Some((id.clone(), Medidas::de(item, &aba.sheet, original)?))
    }

    pub(super) fn evento_formatar_imagem(&mut self, evento: Evento) -> pelicano::Task<Msg> {
        match evento {
            Evento::Abrir => {
                self.imagem_contexto = None;
                self.formatar_imagem.aberto = true;
                self.formatar_imagem.rascunhos.clear();
            }
            Evento::Fechar => {
                self.formatar_imagem.aberto = false;
                self.formatar_imagem.rascunhos.clear();
            }
            Evento::Campo(campo, texto) => {
                let valor = ler(&texto);
                self.formatar_imagem.rascunhos.insert(campo, texto);
                let (Some(v), Some((_, m))) = (valor, self.imagem_para_formatar()) else { return pelicano::Task::none() };
                if !v.is_finite() || (campo != Campo::Rotacao && v <= 0.0) {
                    return pelicano::Task::none();
                }
                if campo == Campo::Rotacao {
                    let giro = v.clamp(-3600.0, 3600.0) % 360.0;
                    self.mudar_imagem(tr!("Imagem girada."), move |item, _| {
                        item["rotation"] = json!(giro);
                        true
                    });
                } else {
                    let (w, h) = novo_tamanho(&m, campo, v);
                    let nova = Posicao { width: w, height: h, ..m.pos };
                    self.mudar_imagem(tr!("Imagem redimensionada."), move |item, sheet| {
                        nova.gravar(item, sheet);
                        true
                    });
                }
            }
            Evento::FixarProporcao(v) => self.mudar_imagem(tr!("Proporção da imagem alterada."), move |item, _| {
                item["lockAspect"] = json!(v);
                true
            }),
            Evento::RelativoOriginal(v) => self.mudar_imagem(tr!("Escala da imagem alterada."), move |item, _| {
                item["relativeToOriginal"] = json!(v);
                true
            }),
            Evento::Redefinir => {
                let Some((_, m)) = self.imagem_para_formatar() else { return pelicano::Task::none() };
                let Some((w, h)) = m.original else {
                    self.status = tr!("Não consegui ler a imagem.").into();
                    return pelicano::Task::none();
                };
                let nova = Posicao {
                    width: f64::from(w).clamp(1.0, LADO_MAXIMO),
                    height: f64::from(h).clamp(1.0, LADO_MAXIMO),
                    ..m.pos
                };
                self.formatar_imagem.rascunhos.clear();
                self.mudar_imagem(tr!("Imagem no tamanho original."), move |item, sheet| {
                    nova.gravar(item, sheet);
                    item["rotation"] = json!(0.0);
                    true
                });
            }
            Evento::Posicionamento(p) => {
                let Some((_, m)) = self.imagem_para_formatar() else { return pelicano::Task::none() };
                // A âncora é refeita onde a imagem está agora: trocar o modo
                // não a tira do lugar.
                self.mudar_imagem(tr!("Propriedades da imagem alteradas."), move |item, sheet| {
                    item["placement"] = json!(p.chave());
                    m.pos.gravar(item, sheet);
                    true
                });
            }
            Evento::Imprimir(v) => self.mudar_imagem(tr!("Propriedades da imagem alteradas."), move |item, _| {
                item["print"] = json!(v);
                true
            }),
            Evento::Bloqueado(v) => self.mudar_imagem(tr!("Propriedades da imagem alteradas."), move |item, _| {
                item["locked"] = json!(v);
                true
            }),
        }
        pelicano::Task::none()
    }

    /// Modo de captura: uma figura em degradê em B3, selecionada, com o
    /// painel (`painel`) ou o menu do botão direito (`menu`) aberto.
    pub(in crate::app) fn preparar_captura_imagem(&mut self, modo: &str) {
        let figura = image::RgbaImage::from_fn(240, 160, |x, y| image::Rgba([(40 + x / 2) as u8, (90 + y / 2) as u8, 200, 255]));
        let mut png = Vec::new();
        if image::DynamicImage::ImageRgba8(figura)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).is_err() { return; }
        self.trocar_selecao(2, 1);
        let _ = self.update(Msg::ImagemLaboratorioCarregada(Ok(Some((png, "exemplo.png".into())))));
        match modo {
            "menu" => {
                self.ponteiro_x = 420.0;
                self.ponteiro_y = 330.0;
                if let Some((_, id)) = self.imagem_selecionada.clone() {
                    let _ = self.update(Msg::ImagemCelula(super::imagem_na_celula::Evento::Contexto(id)));
                }
            }
            _ => { let _ = self.evento_formatar_imagem(Evento::Abrir); }
        }
    }

    fn mudar_imagem(&mut self, mensagem: &str, f: impl FnOnce(&mut Value, &feathersheet_core::planilha::sheet::Sheet) -> bool) {
        let Some((aba_id, id)) = self.imagem_selecionada.clone() else {
            self.status = tr!("Selecione uma imagem.").into();
            return;
        };
        let Some(aba) = self.workbook.aba_mut(self.folha).filter(|a| a.id == aba_id) else { return };
        match imagens_importadas::alterar_item(&mut aba.sheet, &id, f) {
            Ok(true) => {
                self.alterado = true;
                self.invalidar_cache_objetos();
                self.status = mensagem.into();
            }
            Ok(false) => {}
            Err(e) => self.status = e,
        }
    }

    pub(in crate::app) fn view_formatar_imagem(&self) -> Element<'_, Msg> {
        let ev = Msg::FormatarImagem;
        let negrito = pelicano::Font { weight: 600, ..pelicano::Font::DEFAULT };
        let cabecalho = row![
            feather_ui::icones::icone("photo", 18.0),
            text(tr!("Formatar imagem")).size(17).font(negrito).width(Length::FILL),
            feather_ui::tema::botao_fechar(ev(Evento::Fechar)),
        ].spacing(8).align(pelicano::Align::Center);
        let secao = |t: &str| -> Element<'_, Msg> {
            text(t.to_uppercase()).size(10.5).color(COR_TEXTO_MUTED()).font(negrito).into()
        };
        let divisor = || container(Space::new(Length::FILL, Length::Fixed(1.0))).style(|_| pelicano::Style {
            background: Some(pelicano::Background::Color(COR_BORDA())),
            ..Default::default()
        });
        let mut corpo = column![cabecalho].spacing(10);
        match self.imagem_para_formatar() {
            None => {
                corpo = corpo.push(text(tr!("Selecione uma imagem solta na planilha para formatar.")).size(12).color(COR_TEXTO_MUTED()));
            }
            Some((_, m)) => {
                let campo = |rotulo: &str, c: Campo, limites: Limites| -> Element<'_, Msg> {
                    let atual = m.valor(c);
                    // O digitado fica enquanto não vale outra coisa ("1," no meio).
                    let texto = match self.formatar_imagem.rascunhos.get(&c) {
                        Some(r) if ler(r).is_none_or(|v| (v - atual).abs() < 0.01) => r.clone(),
                        _ => escrever((atual * 100.0).round() / 100.0),
                    };
                    let invalido = ler(&texto).is_none();
                    row![
                        text(rotulo.to_owned()).size(12).width(Length::FILL),
                        campo_numero(&texto, "", 112.0, limites, invalido, move |s| ev(Evento::Campo(c, s))),
                    ].spacing(8).align(pelicano::Align::Center).into()
                };
                let cm = Limites { passo: 0.1, min: Some(0.03), max: Some(LADO_MAXIMO / PX_POR_CM) };
                let pct = Limites { passo: 1.0, min: Some(1.0), max: Some(10_000.0) };
                let graus = Limites { passo: 1.0, min: Some(-360.0), max: Some(360.0) };
                let marca = |rotulo: &str, v: bool, f: fn(bool) -> Evento| -> Element<'_, Msg> {
                    checkbox(rotulo.to_owned(), v).size(12).on_toggle(move |b| ev(f(b))).into()
                };
                corpo = corpo
                    .push(secao(tr!("Tamanho")))
                    .push(campo(tr!("Altura (cm)"), Campo::Altura, cm))
                    .push(campo(tr!("Largura (cm)"), Campo::Largura, cm))
                    .push(campo(tr!("Rotação (°)"), Campo::Rotacao, graus))
                    .push(campo(tr!("Altura da escala (%)"), Campo::EscalaAltura, pct))
                    .push(campo(tr!("Largura da escala (%)"), Campo::EscalaLargura, pct))
                    .push(marca(tr!("Fixar taxa de proporção"), m.fixar, Evento::FixarProporcao))
                    .push(marca(tr!("Relativo ao tamanho original da imagem"), m.relativo, Evento::RelativoOriginal));
                let original: Element<'_, Msg> = match m.original {
                    Some((w, h)) => text(trf!("Altura: {} cm   Largura: {} cm",
                        escrever((f64::from(h) / PX_POR_CM * 100.0).round() / 100.0),
                        escrever((f64::from(w) / PX_POR_CM * 100.0).round() / 100.0))).size(12).into(),
                    None => text(tr!("Não consegui ler a imagem.")).size(12).color(COR_TEXTO_MUTED()).into(),
                };
                corpo = corpo
                    .push(text(tr!("Tamanho original")).size(12))
                    .push(original)
                    .push(button(text(tr!("Redefinir")).size(12)).padding([5, 12])
                        .style(feather_ui::tema::estilo_botao_secundario)
                        .on_press_maybe(m.original.map(|_| ev(Evento::Redefinir))))
                    .push(divisor())
                    .push(secao(tr!("Propriedades")));
                for (rotulo, p) in [
                    (tr!("Mover e dimensionar com células"), Posicionamento::MoverEDimensionar),
                    (tr!("Mover mas não dimensionar com células"), Posicionamento::Mover),
                    (tr!("Não mover ou dimensionar com células"), Posicionamento::Fixa),
                ] {
                    corpo = corpo.push(radio(rotulo, p, Some(m.posicionamento), move |p| ev(Evento::Posicionamento(p)))
                        .size(12));
                }
                corpo = corpo
                    .push(marca(tr!("Imprimir objeto"), m.imprimir, Evento::Imprimir))
                    .push(marca(tr!("Bloqueado"), m.bloqueado, Evento::Bloqueado))
                    .push(text(tr!("Bloqueada, a imagem não se move com a planilha protegida.")).size(11).color(COR_TEXTO_MUTED()));
            }
        }
        container(feather_ui::tema::rolagem(corpo).height(Length::FILL))
            .padding([14, 12])
            .width(Length::Fixed(LARGURA_PAINEL))
            .height(Length::FILL)
            .style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_PAINEL_FAIXA())),
                text_color: Some(COR_TEXTO()),
                border: pelicano::Border { color: COR_BORDA(), width: 1., radius: [0.0; 4] },
                ..Default::default()
            })
            .into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    // PNG 1×1.
    const PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
        0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xfc, 0xcf, 0xc0, 0xf0,
        0x1f, 0x00, 0x05, 0x05, 0x02, 0x00, 0x5f, 0xc8, 0xf1, 0xd2, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
        0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    fn com_imagem() -> (App, String) {
        let mut app = App::default();
        let aba = app.workbook.aba_mut(0).unwrap();
        let id = imagens_importadas::importar(&mut aba.sheet, PNG, "x.png", 1, 1).unwrap();
        let aba_id = aba.id;
        app.imagem_selecionada = Some((aba_id, id.clone()));
        let _ = app.evento_formatar_imagem(Evento::Abrir);
        (app, id)
    }

    fn medidas(app: &App) -> Medidas {
        app.invalidar_cache_objetos();
        app.imagem_para_formatar().unwrap().1
    }

    #[test]
    fn altura_em_cm_mantem_a_proporcao_e_desfaz() {
        let (mut app, _) = com_imagem();
        let _ = app.evento_formatar_imagem(Evento::Campo(Campo::Largura, "2".into()));
        let m = medidas(&app);
        assert!((m.valor(Campo::Largura) - 2.0).abs() < 0.03);
        assert_eq!(m.pos.width, m.pos.height, "1×1 com proporção fixa");
        let _ = app.evento_formatar_imagem(Evento::FixarProporcao(false));
        let _ = app.evento_formatar_imagem(Evento::Campo(Campo::Altura, "1".into()));
        let m = medidas(&app);
        assert!((m.valor(Campo::Altura) - 1.0).abs() < 0.03);
        assert!((m.valor(Campo::Largura) - 2.0).abs() < 0.03, "sem proporção a largura fica");
        app.workbook.aba_mut(0).unwrap().sheet.undo();
        assert!((medidas(&app).valor(Campo::Altura) - 2.0).abs() < 0.03, "Ctrl+Z volta a altura");
    }

    #[test]
    fn redefinir_volta_ao_tamanho_original_e_tira_o_giro() {
        let (mut app, _) = com_imagem();
        let _ = app.evento_formatar_imagem(Evento::Campo(Campo::Rotacao, "30".into()));
        let _ = app.evento_formatar_imagem(Evento::Campo(Campo::EscalaLargura, "500".into()));
        assert_eq!(medidas(&app).giro, 30.0);
        let _ = app.evento_formatar_imagem(Evento::Redefinir);
        let m = medidas(&app);
        assert_eq!((m.pos.width, m.pos.height, m.giro), (1.0, 1.0, 0.0));
    }

    #[test]
    fn mover_e_dimensionar_acompanha_a_largura_da_coluna() {
        let (mut app, id) = com_imagem();
        let _ = app.evento_formatar_imagem(Evento::Campo(Campo::Largura, "5".into()));
        let antes = medidas(&app).pos;
        let _ = app.evento_formatar_imagem(Evento::Posicionamento(Posicionamento::MoverEDimensionar));
        assert_eq!(medidas(&app).pos, antes, "trocar o modo não tira do lugar");
        // A coluna A (antes da imagem) cresce 50 px: a imagem anda junto.
        let padrao = crate::grade::largura_padrao_da(&app.workbook.aba(0).unwrap().sheet);
        app.workbook.aba_mut(0).unwrap().sheet.col_widths.insert(0, padrao as u32 + 50);
        let depois = medidas(&app).pos;
        assert!((depois.x - antes.x - 50.0).abs() < 0.5, "{antes:?} → {depois:?}");
        let _ = id;
    }

    #[test]
    fn abrir_o_painel_da_imagem_fecha_o_do_grafico() {
        let (mut app, _) = com_imagem();
        let _ = app.update(Msg::FormatarImagem(Evento::Fechar));
        app.graficos.aberto = true;
        let _ = app.update(Msg::FormatarImagem(Evento::Abrir));
        assert!(app.formatar_imagem.aberto);
        assert!(!app.graficos.aberto, "só um painel lateral por vez");
    }
}
