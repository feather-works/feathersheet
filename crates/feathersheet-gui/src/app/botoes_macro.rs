//! Botões de macro do frontend TypeScript, nativos em Rust/Iced.
//! Os objetos continuam no metadado `Sheet.bt` do FSHZ; nenhuma macro é
//! executada na abertura/na renderização, apenas por clique explícito.
use super::*;
use iced::{mouse, Point, Color, Border, widget::{column, mouse_area}};
use crate::app::imagens_importadas::Alca;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_BOTOES: usize = 128;
const CABECALHO_X: f64 = 90.;
const CABECALHO_Y: f64 = 30.;
static SEQUENCIA_BOTAO: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub(super) struct Editor {
    pub id: Option<String>,
    pub rotulo: String,
    pub macro_nome: String,
}

fn listar(sheet: &feathersheet_core::planilha::sheet::Sheet) -> Result<Vec<Value>, String> {
    let Some(data) = sheet.bt.as_deref() else { return Ok(Vec::new()) };
    let itens: Vec<Value> = serde_json::from_str(data)
        .map_err(|e| trf!("Metadados dos botões inválidos: {e}"))?;
    if itens.len() > MAX_BOTOES { return Err(tr!("Limite de 128 botões por aba").into()); }
    Ok(itens)
}

fn guardar(sheet: &mut feathersheet_core::planilha::sheet::Sheet,
    itens: &[Value]) -> Result<(), String>
{
    if itens.len() > MAX_BOTOES { return Err(tr!("Limite de 128 botões por aba").into()); }
    let dados = serde_json::to_string(itens)
        .map_err(|e| trf!("Falha ao guardar os botões: {e}"))?;
    sheet.alterar_botoes(if itens.is_empty() { None } else { Some(dados) });
    Ok(())
}

/// Posição em pixels lógicos da planilha, compatível com o overlay TS.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Posicao {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
impl Posicao {
    fn ler(v: &Value) -> Option<Self> {
        let p = &v["position"];
        let res = Self {
            x: p["x"].as_f64()?, y: p["y"].as_f64()?,
            width: p["width"].as_f64()?, height: p["height"].as_f64()?,
        };
        ([res.x,res.y,res.width,res.height].iter().all(|v| v.is_finite())
            && res.x >= 0. && res.y >= 0. && res.width >= 40.
            && res.height >= 24. && res.width <= 4096. && res.height <= 4096.)
            .then_some(res)
    }
    fn transformar(self, inicio: Point, atual: Point, zoom: f64, gesto: Gesto) -> Self {
        if !zoom.is_finite() || zoom <= 0. { return self; }
        let dx = f64::from(atual.x - inicio.x) / zoom;
        let dy = f64::from(atual.y - inicio.y) / zoom;
        if !dx.is_finite() || !dy.is_finite() { return self; }
        let (mut x, mut y, mut w, mut h) =
            (self.x, self.y, self.width, self.height);
        match gesto {
            Gesto::Mover => {
                x = (x + dx).max(0.);
                y = (y + dy).max(0.);
            },
            Gesto::Alca(alca) => {
                if matches!(alca, Alca::E|Alca::Ne|Alca::Se) {
                    w = (w + dx).clamp(40., 4096.);
                }
                if matches!(alca, Alca::W|Alca::Nw|Alca::Sw) {
                    w = (w - dx).clamp(40., 4096.);
                    x = self.x + self.width - w;
                }
                if matches!(alca, Alca::S|Alca::Se|Alca::Sw) {
                    h = (h + dy).clamp(24., 4096.);
                }
                if matches!(alca, Alca::N|Alca::Nw|Alca::Ne) {
                    h = (h - dy).clamp(24., 4096.);
                    y = self.y + self.height - h;
                }
            },
        }
        Self {
            x: x.max(0.).round(), y: y.max(0.).round(),
            width: w.round(), height: h.round(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesto { Mover, Alca(Alca) }

/// A roda do mouse sobre um botão ou imagem rola a planilha, como no Excel:
/// a camada de cima não deixa a roda chegar à grade.
pub(super) fn rolar_pela_roda(delta: mouse::ScrollDelta) -> Msg {
    let (x, y) = match delta {
        mouse::ScrollDelta::Lines { x, y } => (x, y),
        mouse::ScrollDelta::Pixels { x, y } => (x / 40.0, y / 40.0),
    };
    Msg::Rolar(-(y * 3.0).round() as i32, -(x * 2.0).round() as i32)
}

#[derive(Debug, Clone)]
pub(super) struct Arraste {
    id: String,
    aba_id: u32,
    inicio: Point,
    base: Posicao,
    atual: Posicao,
    gesto: Gesto,
    deslocou: bool,
}

/// Somente a posição muda; rótulo, macro e campos desconhecidos são preservados.
fn salvar_posicao(sheet: &mut feathersheet_core::planilha::sheet::Sheet,
    id: &str, nova: Posicao) -> Result<bool, String>
{
    let mut itens = listar(sheet)?;
    let item = itens.iter_mut().find(|v| v["id"].as_str() == Some(id))
        .ok_or(tr!("O botão não existe mais."))?;
    let anterior = Posicao::ler(item).ok_or(tr!("Posição do botão inválida."))?;
    if anterior == nova { return Ok(false); }
    for (k,v) in [("x",nova.x),("y",nova.y),
                  ("width",nova.width),("height",nova.height)] {
        item["position"][k] = json!(v);
    }
    guardar(sheet, &itens)?;
    Ok(true)
}

impl App {
    pub(super) fn novo_botao_macro(&mut self) {
        if !self.confirmar() { return; }
        let Some(primeira) = self.macro_catalogo.macros.first() else {
            self.status = tr!("Crie e salve uma macro JavaScript antes de inserir um botão.").into();
            return;
        };
        self.botao_macro_modal = Some(Editor {
            id: None,
            rotulo: tr!("Executar macro").into(),
            macro_nome: primeira.name.clone(),
        });
    }

    pub(super) fn editar_botao_macro(&mut self, id: &str) {
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        match listar(&aba.sheet) {
            Ok(lista) => {
                if let Some(item) = lista.iter().find(|v| v["id"].as_str() == Some(id)) {
                    self.botao_macro_modal = Some(Editor {
                        id: Some(id.to_owned()),
                        rotulo: item["label"].as_str().unwrap_or(tr!("Botão")).to_owned(),
                        macro_nome: item["macroName"].as_str().unwrap_or("").to_owned(),
                    });
                }
            },
            Err(erro) => self.status = erro,
        }
    }

    pub(super) fn guardar_botao_macro(&mut self) {
        if !self.confirmar() { return; }
        let Some(editor) = self.botao_macro_modal.clone() else { return };
        if self.macro_catalogo.obter(&editor.macro_nome).is_none() {
            self.status = tr!("A macro selecionada não existe mais. Salve o módulo antes.").into();
            return;
        }
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let mut itens = match listar(&aba.sheet) {
            Ok(itens) => itens,
            Err(erro) => { self.status = erro; return }
        };
        let novo = editor.id.is_none();
        let indice = if let Some(id) = &editor.id {
            let Some(pos) = itens.iter().position(|v| v["id"].as_str() == Some(id)) else {
                self.status = tr!("O botão não existe mais nesta aba.").into();
                return;
            };
            pos
        } else {
            if itens.len() >= MAX_BOTOES {
                self.status = tr!("Limite de 128 botões por aba.").into();
                return;
            }
            let id = format!("rust-btn-{}-{}",std::process::id(),
                SEQUENCIA_BOTAO.fetch_add(1,Ordering::Relaxed));
            itens.push(json!({
                "id": id,
                "label": "Executar macro",
                "macroName": editor.macro_nome,
                "position": {
                    "x": f64::from(crate::grade::modelo::inicio_x(&self.workbook, self.folha, self.coluna)) + 12.,
                    "y": f64::from(crate::grade::modelo::inicio_y(&self.workbook, self.folha, self.linha)) + 4.,
                    "width": 145.,
                    "height": 31.
                }
            }));
            itens.len() - 1
        };
        let rotulo = editor.rotulo.trim();
        itens[indice]["label"] = json!(if rotulo.is_empty() { "Botão" } else { rotulo });
        itens[indice]["macroName"] = json!(editor.macro_nome);
        if let Some(aba) = self.workbook.aba_mut(self.folha) {
            match guardar(&mut aba.sheet, &itens) {
                Ok(()) => {
                    self.alterado = true;
                    self.botao_macro_modal = None;
                    self.status = if novo { tr!("Botão de macro inserido na planilha.").into() }
                        else { tr!("Botão de macro atualizado.").into() };
                },
                Err(erro) => self.status = erro,
            }
        }
    }

    pub(super) fn excluir_botao_macro(&mut self) {
        let Some(editor) = self.botao_macro_modal.as_ref() else { return };
        let Some(id) = editor.id.clone() else {
            self.botao_macro_modal = None;
            return
        };
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let mut itens = match listar(&aba.sheet) {
            Ok(itens) => itens,
            Err(erro) => { self.status = erro; return }
        };
        let antes = itens.len();
        itens.retain(|v| v["id"].as_str() != Some(id.as_str()));
        if itens.len() == antes {
            self.status = tr!("O botão não existe mais.").into();
            return;
        }
        if let Some(aba) = self.workbook.aba_mut(self.folha) {
            match guardar(&mut aba.sheet, &itens) {
                Ok(()) => {
                    self.alterado = true;
                    self.botao_macro_modal = None;
                    self.botao_macro_selecionado = None;
                    self.arraste_botao_macro = None;
                    self.status = tr!("Botão removido sem excluir a macro (Ctrl+Z desfaz).").into();
                },
                Err(erro) => self.status = erro,
            }
        }
    }

    pub(super) fn apagar_botao_macro_selecionado(&mut self) {
        let Some((aba_id, id)) = self.botao_macro_selecionado.clone() else { return };
        if self.workbook.aba(self.folha).is_none_or(|a| a.id != aba_id) {
            self.botao_macro_selecionado = None;
            return;
        }
        self.editar_botao_macro(&id);
        if self.botao_macro_modal.as_ref().is_some_and(|e| e.id.as_deref() == Some(id.as_str())) {
            self.excluir_botao_macro();
        }
    }

    pub(super) fn executar_botao_macro(&mut self, id: String) {
        if !self.confirmar() || !self.macros_liberadas() { return; }
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let lista = match listar(&aba.sheet) {
            Ok(lista) => lista,
            Err(erro) => { self.status = erro; return }
        };
        let item = lista.iter().find(|v| v["id"].as_str() == Some(id.as_str()));
        let rotulo = item.and_then(|v| v["label"].as_str()).unwrap_or(tr!("Botão")).to_owned();
        let Some(nome) = item.and_then(|v| v["macroName"].as_str()) else {
            self.status = tr!("Este botão não possui uma macro atribuída.").into();
            return;
        };
        let Some(macro_atual) = self.macro_catalogo.obter(nome) else {
            self.status = trf!("Macro \"{nome}\" não encontrada na biblioteca.");
            return;
        };
        let codigo = macro_atual.code.clone();
        let nome = macro_atual.name.clone();
        // A API de macros compartilhada já faz commit/rollback do host.
        let Some(resultado) = self.rodar_macro(historico_macros::GatilhoMacro::Botao { rotulo }, &nome, &codigo, Vec::new()) else { return };
        self.macro_saida = resultado.saida.join("\n");
        self.status = if let Some(erro) = resultado.erro {
            trf!("Erro ao executar {nome}: {erro}. Escritas revertidas.")
        } else { trf!("Macro {nome} executada pelo botão.") };
        self.trocar_selecao(self.linha, self.coluna);
    }

    /// Mouse-down apenas inicia o gesto; o clique executa a macro no mouse-up,
    /// se não houve deslocamento de ao menos 4 px.
    pub(super) fn pressionar_botao_macro(&mut self, id: String, gesto: Gesto) {
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let Ok(lista) = listar(&aba.sheet) else { return };
        let Some(pos) = lista.iter().find(|v| v["id"].as_str() == Some(id.as_str()))
            .and_then(Posicao::ler) else { return };
        self.botao_macro_selecionado = Some((aba.id, id.clone()));
        self.imagem_selecionada = None;
        self.arraste_imagem = None;
        self.arraste_botao_macro = Some(Arraste {
            id, aba_id: aba.id, inicio: self.graficos.cursor,
            base: pos, atual: pos, gesto, deslocou: false,
        });
    }

    pub(super) fn mover_cursor_botao_macro(&mut self, cursor: Point) {
        if let Some(drag) = &mut self.arraste_botao_macro {
            let dx = f64::from(cursor.x - drag.inicio.x);
            let dy = f64::from(cursor.y - drag.inicio.y);
            if dx.hypot(dy) >= 4. { drag.deslocou = true; }
            if drag.deslocou {
                drag.atual = drag.base.transformar(drag.inicio, cursor,
                    f64::from(self.config.zoom) / 100., drag.gesto);
            }
        }
    }

    pub(super) fn soltar_botao_macro(&mut self) {
        let Some(drag) = self.arraste_botao_macro.take() else { return };
        if !drag.deslocou && drag.gesto == Gesto::Mover {
            self.executar_botao_macro(drag.id);
            return;
        }
        let Some(aba) = self.workbook.aba_mut(self.folha)
            .filter(|a| a.id == drag.aba_id) else { return };
        match salvar_posicao(&mut aba.sheet, &drag.id, drag.atual) {
            Ok(true) => {
                self.alterado = true;
                self.status = match drag.gesto {
                    Gesto::Mover => tr!("Botão movido; Ctrl+Z desfaz."),
                    Gesto::Alca(_) => tr!("Botão redimensionado; Ctrl+Z desfaz."),
                }.into();
            },
            Ok(false) => {},
            Err(erro) => self.status = erro,
        }
    }

    /// A grade permanece interativa: somente as áreas dos botões recebem clique.
    pub(super) fn grade_com_botoes_macro<'a>(
        &'a self, grade: Element<'a, Msg>,
    ) -> Element<'a, Msg> {
        let Some(aba) = self.workbook.aba(self.folha) else { return grade };
        let Ok(lista) = listar(&aba.sheet) else { return grade };
        if lista.is_empty() { return grade; }
        let z = f64::from(self.config.zoom) / 100.;
        let (fixas_r, fixas_c) = self.congelamento_atual();
        // Régua real da aba (larguras e alturas próprias), a mesma da grade.
        use crate::grade::modelo::{inicio_x, inicio_y};
        let xf = f64::from(inicio_x(&self.workbook, self.folha, fixas_c));
        let yf = f64::from(inicio_y(&self.workbook, self.folha, fixas_r));
        let dx = f64::from(inicio_x(&self.workbook, self.folha, self.coluna_inicio));
        let dy = f64::from(inicio_y(&self.workbook, self.folha, self.linha_inicio));
        let mut camadas: Vec<Element<'a, Msg>> = vec![grade];
        for item in lista {
            let Some(id) = item["id"].as_str() else { continue };
            let Some(mut pos) = Posicao::ler(&item) else { continue };
            let selecionado = self.botao_macro_selecionado.as_ref()
                .is_some_and(|(aba_id, escolhido)| *aba_id == aba.id && escolhido == id);
            if let Some(drag) = &self.arraste_botao_macro {
                if drag.aba_id == aba.id && drag.id == id {
                    pos = drag.atual;
                }
            }
            let w = pos.width * z;
            let h = pos.height * z;
            let x = ((if pos.x < xf { pos.x } else { pos.x - dx + xf }) + CABECALHO_X) * z;
            let y = ((if pos.y < yf { pos.y } else { pos.y - dy + yf }) + CABECALHO_Y) * z;
            // Botão que subiu (ou foi para a esquerda) além das células não
            // é desenhado por cima dos cabeçalhos de linha e coluna.
            if !x.is_finite() || !y.is_finite() || x < CABECALHO_X * z || y < CABECALHO_Y * z
                || x > 10_000. || y > 10_000. { continue; }
            // O corpo inteiro captura o press e aguarda mouse-up global antes
            // de decidir entre clique/executar e mover. Não executa no drag.
            let rotulo = item["label"].as_str().unwrap_or(tr!("Botão")).to_owned();
            // Cor da forma do Excel ("#548135"); sem ela, o verde do botão.
            let (fundo, borda) = item["color"].as_str().and_then(cor_hex)
                .map(|c| (c, Color::from_rgb(c.r * 0.82, c.g * 0.82, c.b * 0.82)))
                .unwrap_or((Color::from_rgb8(34, 124, 92), Color::from_rgb8(28, 102, 79)));
            let corpo = mouse_area(container(text(rotulo).size(12))
                .center_x(Length::Fill).center_y(Length::Fill)
                .width(Length::Fixed(w as f32))
                .height(Length::Fixed(h as f32))
                .style(move |_| container::Style {
                    background: Some(fundo.into()),
                    border: Border { color: borda,
                        width: 1., radius: 7.0.into() },
                    text_color: Some(Color::WHITE),
                    ..Default::default()
                }))
                .on_press(Msg::BotaoMacroPressionar(id.to_owned(), Gesto::Mover))
                // Editar pelo botão direito, como no Excel: nada fica grudado ao lado do botão.
                .on_right_press(Msg::BotaoMacroEditar(id.to_owned()))
                .on_scroll(rolar_pela_roda)
                .interaction(mouse::Interaction::Grab);
            let conteudo: Element<'_, Msg> = if selecionado {
                let mut alcas: Vec<Element<'_, Msg>> = vec![corpo.into()];
                for alca in Alca::TODAS {
                    let (ax, ay) = alca.origem(w, h);
                    let handle = container(Space::new(Length::Fixed(10.),
                        Length::Fixed(10.))).style(|_| container::Style {
                        background: Some(Color::WHITE.into()),
                        border: Border {
                            color: Color::from_rgb8(37, 99, 235),
                            width: 1., radius: 2.0.into(),
                        },
                        ..Default::default()
                    });
                    let alca_widget = mouse_area(handle)
                        .on_press(Msg::BotaoMacroPressionar(id.to_owned(), Gesto::Alca(alca)))
                        .interaction(alca.cursor());
                    alcas.push(column![
                        Space::with_height(Length::Fixed(ay as f32)),
                        row![
                            Space::with_width(Length::Fixed(ax as f32)),
                            alca_widget,
                        ],
                    ].into());
                }
                stack(alcas).into()
            } else { corpo.into() };
            camadas.push(column![
                Space::with_height(Length::Fixed(y as f32)),
                row![
                    Space::with_width(Length::Fixed(x as f32)),
                    conteudo,
                ],
            ].into());
        }
        stack(camadas).height(Length::Fill).width(Length::Fill).into()
    }

    /// Caixa de edição sem HTML/DOM nem propriedades fixas no rodapé.
    pub(super) fn sobrepor_editor_botao_macro<'a>(
        &'a self, base: Element<'a, Msg>,
    ) -> Element<'a, Msg> {
        let Some(editor) = &self.botao_macro_modal else { return base };
        let mut conteudo = column![
            text(if editor.id.is_some() { tr!("Editar botão de macro") }
                else { tr!("Inserir botão de macro") }).size(19),
            text(tr!("Rótulo do botão")).size(12),
            text_input(tr!("Executar macro"), &editor.rotulo)
                .on_input(Msg::BotaoMacroRotulo),
            text(tr!("Macro a executar")).size(12),
        ].spacing(10);
        for item in self.macro_catalogo.macros.iter().take(50) {
            let nome = item.name.clone();
            conteudo = conteudo.push(
                button(text(format!("{} {}", if nome == editor.macro_nome { "●" } else { "○" }, nome)))
                    .width(Length::Fill)
                    .style(estilo_botao_secundario)
                    .on_press(Msg::BotaoMacroEscolher(nome)),
            );
        }
        let mut botoes = row![
            button(tr!("Cancelar")).on_press(Msg::BotaoMacroCancelar)
                .style(estilo_botao_secundario),
            button(tr!("Salvar botão")).on_press(Msg::BotaoMacroSalvar)
                .style(estilo_botao_primario),
        ].spacing(10);
        if editor.id.is_some() {
            botoes = botoes.push(button(tr!("Excluir botão"))
                .on_press(Msg::BotaoMacroExcluir).style(estilo_botao_secundario));
        }
        conteudo = conteudo.push(botoes);
        let painel = container(scrollable(conteudo.padding(17))
            .height(Length::Fixed(360.)))
            .width(Length::Fixed(420.))
            .style(estilo_painel_faixa);
        stack![
            base,
            container(painel).center_x(Length::Fill).center_y(Length::Fill)
                .width(Length::Fill).height(Length::Fill),
        ].height(Length::Fill).into()
    }
}

/// `#RRGGBB` → cor do Iced.
fn cor_hex(t: &str) -> Option<Color> {
    let t = t.strip_prefix('#')?;
    if t.len() != 6 { return None; }
    let n = u32::from_str_radix(t, 16).ok()?;
    Some(Color::from_rgb8((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn cria_botao_preserva_js_e_metadata_fshz() {
        let mut app = App::default();
        // O catálogo salvo na máquina não entra no teste.
        app.macro_catalogo = Default::default();
        app.macro_catalogo.macros.push(feather_macros::Macro {
            name: "TesteBotao".into(), code: "log('ok')".into(), auto: false,
        });
        app.novo_botao_macro();
        assert!(app.botao_macro_modal.is_some());
        app.guardar_botao_macro();
        let bt = app.workbook.aba(0).unwrap().sheet.bt.clone().unwrap();
        assert!(bt.contains("TesteBotao"));
        let bytes = feathersheet_io::da_workbook::pasta_gravar_bytes(
            &app.workbook, 0).unwrap();
        let lida = feathersheet_io::fshz_read::ler_fshz(&bytes).unwrap();
        let aberto = feathersheet_io::para_workbook::para_workbook(&lida);
        assert_eq!(aberto.aba(0).unwrap().sheet.bt.as_deref(), Some(bt.as_str()));
    }

    #[test]
    fn arrastar_botao_nao_executa_macro_e_desfaz_posicao() {
        let mut app = App::default();
        // O catálogo salvo na máquina não entra no teste.
        app.macro_catalogo = Default::default();
        app.macro_catalogo.macros.push(feather_macros::Macro {
            name: "Mover".into(), code: "sheet.range('A1').value=99;".into(), auto: false,
        });
        app.novo_botao_macro();
        app.guardar_botao_macro();
        let antes = app.workbook.aba(0).unwrap().sheet.bt.clone().unwrap();
        let itens: Vec<Value> = serde_json::from_str(&antes).unwrap();
        let id = itens[0]["id"].as_str().unwrap().to_string();
        app.graficos.cursor = Point::new(100., 100.);
        app.pressionar_botao_macro(id.clone(), Gesto::Mover);
        app.mover_cursor_botao_macro(Point::new(155., 119.));
        assert!(app.arraste_botao_macro.as_ref().unwrap().deslocou);
        app.soltar_botao_macro();
        let depois = app.workbook.aba(0).unwrap().sheet.bt.as_ref().unwrap();
        let lista: Vec<Value> = serde_json::from_str(depois).unwrap();
        assert_eq!(lista[0]["position"]["x"], itens[0]["position"]["x"].as_f64().unwrap() + 55.);
        assert_eq!(lista[0]["position"]["y"], itens[0]["position"]["y"].as_f64().unwrap() + 19.);
        // Uma operação de arrastar nunca dispara o código JavaScript.
        assert!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0).is_empty());
        app.historico(false);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.bt.as_deref(), Some(antes.as_str()));
        app.historico(true);
        assert_ne!(app.workbook.aba(0).unwrap().sheet.bt.as_deref(), Some(antes.as_str()));
    }

    #[test]
    fn oito_alcas_mantem_tamanhos_minimos_sem_negativos() {
        let base = Posicao { x: 50., y: 60., width: 140., height: 50. };
        for alca in Alca::TODAS {
            let nova = base.transformar(
                Point::new(100., 100.), Point::new(700., 600.),
                1., Gesto::Alca(alca));
            assert!(nova.x >= 0. && nova.y >= 0.);
            assert!((40. ..= 4096.).contains(&nova.width));
            assert!((24. ..= 4096.).contains(&nova.height));
        }
        let nova = base.transformar(
            Point::new(100., 100.), Point::new(150., 130.),
            1., Gesto::Alca(Alca::Se));
        assert_eq!(nova, Posicao {
            x: 50., y: 60., width: 190., height: 80.,
        });
    }

    #[test]
    fn click_dispara_macro_mas_pequena_tremida_nao_move_botao() {
        let mut app = App::default();
        // O catálogo salvo na máquina não entra no teste.
        app.macro_catalogo = Default::default();
        app.macro_catalogo.macros.push(feather_macros::Macro {
            name: "Clique".into(), code: "sheet.range('B1').value=21;".into(), auto: false,
        });
        app.novo_botao_macro();
        app.guardar_botao_macro();
        let antes = app.workbook.aba(0).unwrap().sheet.bt.clone().unwrap();
        let lista: Vec<Value> = serde_json::from_str(&antes).unwrap();
        let id = lista[0]["id"].as_str().unwrap().to_string();
        app.graficos.cursor = Point::new(100., 100.);
        app.pressionar_botao_macro(id, Gesto::Mover);
        app.mover_cursor_botao_macro(Point::new(102., 101.));
        app.soltar_botao_macro();
        assert_eq!(app.workbook.valor(0, 0, 1), feathersheet_core::Value::Number(21.));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.bt.as_deref(), Some(antes.as_str()));
    }

    #[test]
    fn clicar_executa_somente_por_acao_explicita_e_excluir_nao_apaga_modulo() {
        let mut app = App::default();
        // O catálogo salvo na máquina não entra no teste.
        app.macro_catalogo = Default::default();
        app.macro_catalogo.macros.push(feather_macros::Macro {
            name: "TesteBotao".into(), code: "sheet.range('A1').value=42;".into(), auto: false,
        });
        app.novo_botao_macro();
        app.guardar_botao_macro();
        let bt = app.workbook.aba(0).unwrap().sheet.bt.clone().unwrap();
        let itens: Vec<Value> = serde_json::from_str(&bt).unwrap();
        let id = itens[0]["id"].as_str().unwrap().to_string();
        assert!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0).is_empty());
        app.executar_botao_macro(id.clone());
        assert_eq!(app.workbook.valor(0, 0, 0),
            feathersheet_core::Value::Number(42.));
        app.editar_botao_macro(&id);
        app.excluir_botao_macro();
        assert!(app.workbook.aba(0).unwrap().sheet.bt.is_none());
        assert!(app.macro_catalogo.obter("TesteBotao").is_some());
    }
}
