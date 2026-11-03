//! Controles de formulário do Excel (o `tipo: "controle"` dos objetos da aba): o desenho com a
//! cara do Excel e o clique — botão roda a macro, botão de girar e barra de rolagem andam o valor
//! da célula ligada, caixa de seleção e opção marcam, lista e caixa de combinação escolhem o item.
//! O modelo e a leitura/gravação do `.xlsx` moram no `feathersheet-io` (`objetos_aba`,
//! `xlsx_read/controles.rs`, `xlsx_write_controles.rs`).
use super::*;
use crate::compat::canvas as cv;
use crate::compat::canvas::{Frame, Path, Stroke};
use crate::compat::{Rectangle, Retangulo as _};
use feathersheet_io::objetos_aba::{ControleFormulario, TipoControleForm as T};
use pelicano::{Color, Point, Size};

/// Altura de uma linha da lista e da combinação aberta, em px lógicos (zoom 100 %).
pub(super) const LINHA_LISTA: f32 = 17.0;
/// Teto de itens lidos do intervalo de entrada.
const MAX_ITENS: usize = 2_000;

const FACE: Color = Color::rgb(0.94, 0.94, 0.94);
const BORDA: Color = Color::rgb(0.68, 0.68, 0.68);
const TEXTO: Color = Color::rgb(0.0, 0.0, 0.0);
const BRANCO: Color = Color::WHITE;
const ESCOLHA: Color = Color::rgb(0.0, 0.47, 0.84);

/// O que o desenho precisa do controle, com o valor da célula ligada já lido.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ControleTela {
    pub tipo: T,
    pub valor: i64,
    pub min: i64,
    pub max: i64,
    pub marcado: bool,
    pub horizontal: bool,
    pub itens: Vec<String>,
    /// Item escolhido na lista/combinação (base 0).
    pub escolhido: Option<usize>,
    /// Combinação com a lista aberta (desenhada abaixo da caixa).
    pub aberto: bool,
    /// Altura da caixa em px lógicos (com a lista aberta, o retângulo do item é maior).
    pub altura_base: f32,
    /// Cores e linhas escolhidas (`None` = visual padrão do Excel).
    pub fundo: Option<Color>,
    pub linha: Option<Color>,
}

/// Lê `$C$8` ou `'Aba'!$C$8:$C$20` como (aba, retângulo). `None` se não for referência.
fn referencia(app: &App, texto: &str) -> Option<(usize, u32, u32, u32, u32)> {
    use feathersheet_core::formula::ast::Node;
    let no = feathersheet_core::formula::parser::parse_formula(texto.trim().trim_start_matches('=')).ok()?;
    let aba = |nome: &Option<String>| nome.as_deref().map_or(Some(app.folha), |n| app.workbook.indice_por_nome(n));
    match &no {
        Node::Ref { row, col, sheet } => Some((aba(sheet)?, *row, *col, *row, *col)),
        Node::Range { r0, c0, r1, c1, sheet } => Some((aba(sheet)?, *r0.min(r1), *c0.min(c1), *r0.max(r1), *c0.max(c1))),
        _ => None,
    }
}

/// O número da célula ligada (`None` vazia ou texto que não é número).
pub(super) fn numero_ligado(app: &App, c: &ControleFormulario) -> Option<f64> {
    let (aba, r, col, ..) = referencia(app, c.celula.as_deref()?)?;
    match app.workbook.valor(aba, r, col) {
        feathersheet_core::Value::Number(n) => Some(n),
        feathersheet_core::Value::Bool(b) => Some(if b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

impl App {
    /// O controle como a tela mostra: a célula ligada manda (o Excel também lê dela).
    pub(super) fn controle_tela(&self, o: &ObjetoSolto, opcoes: &[(u64, Option<String>)], regua_altura: f32) -> Option<ControleTela> {
        let c = o.controle.as_ref()?;
        let ligado = numero_ligado(self, c);
        let valor = ligado.map(|n| n.round() as i64).or(c.valor).unwrap_or(c.min.unwrap_or(0));
        // Arrastando a caixa da barra: mostra onde ela está, antes de gravar.
        let valor = self.objetos.barra_arrastada.filter(|(i, _)| *i == o.id).map_or(valor, |(_, v)| v);
        let marcado = match c.tipo {
            T::Caixa => ligado.map_or(c.marcado == Some(1), |n| n != 0.0),
            // Opção: a célula do grupo guarda a posição (1, 2…) da marcada.
            T::Opcao => {
                let pos = opcoes.iter().position(|(id, _)| *id == o.id).map(|p| p as f64 + 1.0);
                let celula_do_grupo = opcoes.iter().find_map(|(_, cel)| cel.clone());
                let n = celula_do_grupo.and_then(|cel| {
                    let (aba, r, col, ..) = referencia(self, &cel)?;
                    match self.workbook.valor(aba, r, col) { feathersheet_core::Value::Number(n) => Some(n), _ => None }
                });
                match n { Some(n) => pos == Some(n), None => c.marcado == Some(1) }
            }
            _ => false,
        };
        let itens: Vec<String> = c.faixa.as_deref().and_then(|f| referencia(self, f)).map(|(aba, r0, c0, r1, c1)| {
            (r0..=r1).flat_map(|r| (c0..=c1).map(move |cc| (r, cc))).take(MAX_ITENS)
                .map(|(r, cc)| {
                    let estilo = self.workbook.aba(aba).and_then(|a| a.sheet.get_style(r, cc));
                    feathersheet_core::planilha::bloco::texto_formatado(&self.workbook.valor(aba, r, cc), estilo)
                }).collect()
        }).unwrap_or_default();
        let escolhido = matches!(c.tipo, T::Lista | T::Combo).then(|| valor - 1).filter(|v| *v >= 0).map(|v| v as usize).filter(|v| *v < itens.len());
        Some(ControleTela {
            tipo: c.tipo, valor, min: c.min.unwrap_or(0), max: c.max.unwrap_or(100), marcado,
            horizontal: c.horizontal, itens, escolhido,
            aberto: c.tipo == T::Combo && self.objetos.combo_aberto == Some(o.id),
            altura_base: regua_altura,
            // Só `#rrggbb` vira cor na tela; cor com nome do VML só atravessa o arquivo.
            fundo: c.fundo.as_deref().filter(|h| h.starts_with('#')).map(|h| super::camada::cor_hex(h, 0.0)),
            linha: c.linha.as_deref().filter(|h| h.starts_with('#')).map(|h| super::camada::cor_hex(h, 0.0)),
        })
    }

    /// Clique num controle em `(x, y)`, px lógicos a partir do canto de cima à esquerda.
    /// Arrasto da caixa da barra de rolagem: enquanto arrasta, só a caixa anda; ao soltar,
    /// o valor vai para a célula (um Ctrl+Z). Começou fora da caixa: não faz nada.
    pub(super) fn arrastar_barra(&mut self, id: u64, de: (f32, f32), ate: (f32, f32), solto: bool) {
        let Some((_, lidos)) = self.objetos_da_aba() else { return };
        let Some(o) = lidos.obter(id).cloned() else { return };
        let Some(c) = o.controle.clone().filter(|c| c.tipo == T::Rolagem) else { return };
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let ret = o.retangulo(&super::ReguaAba(&aba.sheet));
        let atual = numero_ligado(self, &c).map(|n| n.round() as i64).or(c.valor);
        let v = valor_arrastado(&c, atual, (ret.largura.max(1.0), ret.altura.max(1.0)), de, ate);
        match (v, solto) {
            (Some(v), false) => self.objetos.barra_arrastada = Some((id, v)),
            (Some(v), true) => {
                self.objetos.barra_arrastada = None;
                if Some(v) != atual { self.gravar_valor_do_controle(&lidos, id, &c, v); }
            }
            (None, _) => self.objetos.barra_arrastada = None,
        }
    }

    pub(super) fn clicar_controle(&mut self, id: u64, x: f32, y: f32) {
        let Some((_, lidos)) = self.objetos_da_aba() else { return };
        let Some(o) = lidos.obter(id).cloned() else { return };
        let Some(c) = o.controle.clone() else { return };
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let ret = o.retangulo(&super::ReguaAba(&aba.sheet));
        let (w, h) = (ret.largura.max(1.0), ret.altura.max(1.0));
        let aberto = self.objetos.combo_aberto.take() == Some(id);
        let atual = numero_ligado(self, &c).map(|n| n.round() as i64).or(c.valor);
        let novo: Option<i64> = match c.tipo {
            T::Botao | T::Rotulo | T::Grupo => None,
            T::Girar => {
                // Na vertical a metade de cima soma; deitado, a da direita.
                let soma = if h >= w { y < h / 2.0 } else { x > w / 2.0 };
                Some(ControleFormulario { valor: atual, ..c.clone() }.andar(if soma { 1 } else { -1 }, false))
            }
            T::Rolagem => {
                let (pos, comp) = if c.horizontal { (x, w) } else { (y, h) };
                let seta = if c.horizontal { h } else { w }.min(comp / 3.0);
                let base = ControleFormulario { valor: atual, ..c.clone() };
                let (min, max) = (c.min.unwrap_or(0), c.max.unwrap_or(100));
                let fracao = ((atual.unwrap_or(min) - min) as f32 / (max - min).max(1) as f32).clamp(0.0, 1.0);
                let polegar = seta + fracao * (comp - 3.0 * seta);
                Some(if pos < seta { base.andar(-1, false) }
                    else if pos > comp - seta { base.andar(1, false) }
                    else if pos < polegar { base.andar(-1, true) }
                    else if pos > polegar + seta { base.andar(1, true) }
                    else { atual.unwrap_or(min) })
            }
            T::Caixa => {
                let marcado = atual.map_or(c.marcado == Some(1), |n| n != 0);
                Some(if marcado { 0 } else { 1 })
            }
            T::Opcao => {
                let opcoes = opcoes_da_aba(&lidos);
                opcoes.iter().position(|(i, _)| *i == id).map(|p| p as i64 + 1)
            }
            T::Lista => Some((y / LINHA_LISTA).floor() as i64 + 1),
            T::Combo if aberto && y > h => Some(((y - h) / LINHA_LISTA).floor() as i64 + 1),
            T::Combo => {
                if !aberto { self.objetos.combo_aberto = Some(id); }
                None
            }
        };
        if let Some(v) = novo {
            self.gravar_valor_do_controle(&lidos, id, &c, v);
        }
        if o.macro_nome.is_some() && !(c.tipo == T::Combo && novo.is_none()) {
            self.executar_macro_da_forma(id);
        }
    }

    /// Controle novo perto da célula ativa, no tamanho que o Excel usa, já selecionado
    /// (um Ctrl+Z); o botão direito > Formatar controle liga a célula.
    pub(super) fn inserir_controle(&mut self, tipo: T) {
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let Some((aba_id, lidos)) = self.objetos_da_aba() else { return };
        let regua = super::ReguaAba(&aba.sheet);
        let (x, y) = (regua.inicio_coluna(self.coluna) + 4.0, regua.inicio_linha(self.linha) + 4.0);
        let ((w, h), texto) = match tipo {
            T::Botao => ((96.0, 28.0), Some("Botão")),
            T::Caixa => ((110.0, 20.0), Some("Caixa de seleção")),
            T::Opcao => ((110.0, 20.0), Some("Botão de opção")),
            T::Rotulo => ((80.0, 20.0), Some("Rótulo")),
            T::Grupo => ((150.0, 100.0), Some("Caixa de grupo")),
            T::Lista => ((110.0, 80.0), None),
            T::Combo => ((110.0, 22.0), None),
            T::Girar => ((20.0, 40.0), None),
            T::Rolagem => ((20.0, 100.0), None),
        };
        let mut objetos = (*lidos).clone();
        let mut novo = ObjetoSolto::nova_forma(objetos.proximo_id(), "retangulo", Retangulo::novo(x, y, w, h), &regua);
        novo.tipo = modelo::TipoObjeto::Controle;
        novo.aparencia = modelo::Aparencia::default();
        novo.texto = texto.map(|t| tr!(t).to_owned());
        novo.controle = Some(ControleFormulario::novo(tipo));
        let Some(id) = objetos.adicionar(novo) else {
            self.status = trf!("Limite de {} objetos por aba.", modelo::MAX_OBJETOS);
            return;
        };
        self.objetos.selecionado = Some((aba_id, id));
        self.gravar_objetos(&objetos, tr!("Controle inserido (botão direito > Formatar controle liga a célula).").into());
    }

    /// O valor novo vai para a célula ligada (um Ctrl+Z, como digitar) e fica guardado no controle.
    pub(super) fn gravar_valor_do_controle(&mut self, lidos: &ObjetosAba, id: u64, c: &ControleFormulario, v: i64) {
        let opcoes = opcoes_da_aba(lidos);
        let celula = if c.tipo == T::Opcao { opcoes.iter().find_map(|(_, cel)| cel.clone()) } else { c.celula.clone() };
        // Célula e controle num Ctrl+Z só quando a célula é desta aba.
        // shortcut: célula ligada em outra aba fica no histórico daquela aba; juntar se alguém reclamar.
        let folha = self.folha;
        let destino = celula.as_deref().and_then(|t| referencia(self, t));
        let mesma_aba = destino.is_some_and(|(aba, ..)| aba == folha);
        if mesma_aba { if let Some(a) = self.workbook.aba_mut(folha) { a.sheet.abrir_transacao(); } }
        if let Some((aba, r, col, ..)) = destino {
            let texto = match (c.tipo, v) {
                (T::Caixa, 0) => "FALSO".to_string(),
                // Mista: o Excel põe #N/D na célula ligada.
                (T::Caixa, 2) => "#N/D".to_string(),
                (T::Caixa, _) => "VERDADEIRO".to_string(),
                _ => v.to_string(),
            };
            self.workbook.escrever_celula(aba, r, col, texto);
        }
        let mut objetos = lidos.clone();
        for o in objetos.objetos.iter_mut() {
            let Some(oc) = o.controle.as_mut() else { continue };
            if o.id == id {
                if c.tipo == T::Caixa { oc.marcado = Some(v as u8); } else { oc.valor = Some(v); }
                if c.tipo == T::Opcao { oc.marcado = Some(1); }
            } else if c.tipo == T::Opcao && oc.tipo == T::Opcao {
                oc.marcado = Some(0);
            }
        }
        let rotulo = celula.unwrap_or_default();
        self.gravar_objetos(&objetos, if rotulo.is_empty() { String::new() } else { format!("{rotulo} = {v}") });
        if mesma_aba { if let Some(a) = self.workbook.aba_mut(folha) { a.sheet.fechar_transacao(); } }
    }
}

/// O valor da barra com a caixa arrastada de `de` até `ate` (px lógicos do canto do controle);
/// `None` se o arrasto não começou na caixa. A mesma geometria do desenho (`desenhar`).
fn valor_arrastado(c: &ControleFormulario, atual: Option<i64>, (w, h): (f32, f32), de: (f32, f32), ate: (f32, f32)) -> Option<i64> {
    let (pos0, pos, comp, grosso) = if c.horizontal { (de.0, ate.0, w, h) } else { (de.1, ate.1, h, w) };
    let seta = grosso.min(comp / 3.0);
    let (min, max) = (c.min.unwrap_or(0), c.max.unwrap_or(100));
    let (lo, hi) = (min.min(max), max.max(min));
    let curso = (comp - 3.0 * seta).max(1.0);
    let fracao = ((atual.unwrap_or(lo) - lo) as f32 / (hi - lo).max(1) as f32).clamp(0.0, 1.0);
    let polegar = seta + fracao * curso;
    if pos0 < polegar || pos0 > polegar + seta { return None; }
    let nova = ((polegar + pos - pos0 - seta) / curso).clamp(0.0, 1.0);
    Some(lo + (nova * (hi - lo) as f32).round() as i64)
}

/// Os botões de opção da aba (id, célula ligada), na ordem do arquivo: formam um grupo só.
// shortcut: as caixas de grupo não separam grupos ainda; com duas caixas de grupo de opções na mesma aba, separar pelo retângulo da caixa.
pub(super) fn opcoes_da_aba(lidos: &ObjetosAba) -> Vec<(u64, Option<String>)> {
    lidos.objetos.iter().filter_map(|o| o.controle.as_ref().filter(|c| c.tipo == T::Opcao).map(|c| (o.id, c.celula.clone()))).collect()
}

fn texto(f: &mut Frame, t: &str, p: Point, tam: f32, h: pelicano::Align, cor: Color) {
    f.fill_text(cv::Text {
        content: t.to_owned(), position: p, color: cor, size: crate::compat::Pixels(tam),
        horizontal_alignment: h, vertical_alignment: pelicano::Align::Center, ..Default::default()
    });
}

fn texto_l(f: &mut Frame, letra: pelicano::Font, t: &str, p: Point, tam: f32, h: pelicano::Align, cor: Color) {
    f.fill_text(cv::Text {
        content: t.to_owned(), position: p, color: cor, size: crate::compat::Pixels(tam), font: letra,
        horizontal_alignment: h, vertical_alignment: pelicano::Align::Center, ..Default::default()
    });
}

fn caixa(f: &mut Frame, r: Rectangle, fundo: Color, z: f32) {
    f.fill_rectangle(Point::new(r.x, r.y), Size::new(r.width, r.height), fundo);
    f.stroke(&Path::rectangle(Point::new(r.x, r.y), Size::new(r.width, r.height)), Stroke::default().with_color(BORDA).with_width(z.max(0.5)));
}

/// Triângulo de seta (para cima, baixo, esquerda ou direita) centrado em `r`.
fn seta(f: &mut Frame, r: Rectangle, direcao: char) {
    let (c, m) = (r.center(), r.width.min(r.height) * 0.22);
    let pontos = match direcao {
        '^' => [(c.x - m, c.y + m / 2.0), (c.x + m, c.y + m / 2.0), (c.x, c.y - m / 2.0)],
        'v' => [(c.x - m, c.y - m / 2.0), (c.x + m, c.y - m / 2.0), (c.x, c.y + m / 2.0)],
        '<' => [(c.x + m / 2.0, c.y - m), (c.x + m / 2.0, c.y + m), (c.x - m / 2.0, c.y)],
        _ => [(c.x - m / 2.0, c.y - m), (c.x - m / 2.0, c.y + m), (c.x + m / 2.0, c.y)],
    };
    let tri = Path::new(|b| {
        b.move_to(Point::new(pontos[0].0, pontos[0].1));
        b.line_to(Point::new(pontos[1].0, pontos[1].1));
        b.line_to(Point::new(pontos[2].0, pontos[2].1));
        b.close();
    });
    f.fill(&tri, TEXTO);
}

/// Desenha o controle no retângulo de tela `r` (já com o zoom `z`).
pub(super) fn desenhar(f: &mut Frame, ct: &ControleTela, rotulo: Option<&str>, fonte: Option<&modelo::FonteTexto>, alinhar: modelo::AlinharTexto, r: Rectangle, z: f32) {
    let tam = (fonte.and_then(|f| f.tamanho).unwrap_or(11.0) * 4.0 / 3.0 * z).max(5.0);
    let base = Rectangle { height: (ct.altura_base * z).min(r.height), ..r };
    let rotulo = rotulo.unwrap_or("");
    let cor_txt = fonte.and_then(|f| f.cor.as_deref()).map_or(TEXTO, |c| super::camada::cor_hex(c, 0.0));
    let mut letra = pelicano::Font::DEFAULT;
    if fonte.is_some_and(|f| f.negrito) { letra.weight = 700; }
    if fonte.is_some_and(|f| f.italico) { letra.italic = true; }
    // Cores e linhas: fundo e borda escolhidos por baixo do desenho do controle.
    if let Some(c) = ct.fundo { f.fill_rectangle(Point::new(base.x, base.y), Size::new(base.width, base.height), c); }
    if let Some(c) = ct.linha {
        f.stroke(&Path::rectangle(Point::new(base.x, base.y), Size::new(base.width, base.height)), Stroke::default().with_color(c).with_width(z.max(0.5)));
    }
    match ct.tipo {
        T::Botao => {
            caixa(f, base, ct.fundo.unwrap_or(FACE), z);
            let m = 4.0 * z;
            let (x, h) = match alinhar.h {
                modelo::AlinharH::Esquerda => (base.x + m, pelicano::Align::Start),
                modelo::AlinharH::Centro => (base.center_x(), pelicano::Align::Center),
                modelo::AlinharH::Direita => (base.x + base.width - m, pelicano::Align::End),
            };
            let y = match alinhar.v {
                modelo::AlinharV::Topo => base.y + m + tam * 0.6,
                modelo::AlinharV::Meio => base.center_y(),
                modelo::AlinharV::Baixo => base.y + base.height - m - tam * 0.6,
            };
            texto_l(f, letra, rotulo, Point::new(x, y), tam, h, cor_txt);
        }
        T::Rotulo => texto_l(f, letra, rotulo, Point::new(base.x + 2.0 * z, base.center_y()), tam, pelicano::Align::Start, cor_txt),
        T::Grupo => {
            let topo = base.y + tam / 2.0;
            f.stroke(&Path::rectangle(Point::new(base.x, topo), Size::new(base.width, (base.height - tam / 2.0).max(1.0))),
                Stroke::default().with_color(BORDA).with_width(z.max(0.5)));
            if !rotulo.is_empty() {
                let largura = rotulo.chars().count() as f32 * tam * 0.55 + 6.0 * z;
                f.fill_rectangle(Point::new(base.x + 6.0 * z, base.y), Size::new(largura, tam), BRANCO);
                texto_l(f, letra, rotulo, Point::new(base.x + 9.0 * z, topo), tam, pelicano::Align::Start, cor_txt);
            }
        }
        T::Caixa | T::Opcao => {
            let lado = (13.0 * z).min(base.height);
            let q = Rectangle { x: base.x + 2.0 * z, y: base.center_y() - lado / 2.0, width: lado, height: lado };
            if ct.tipo == T::Caixa {
                caixa(f, q, BRANCO, z);
                if ct.marcado {
                    let v = Path::new(|b| {
                        b.move_to(Point::new(q.x + lado * 0.2, q.y + lado * 0.5));
                        b.line_to(Point::new(q.x + lado * 0.42, q.y + lado * 0.75));
                        b.line_to(Point::new(q.x + lado * 0.8, q.y + lado * 0.25));
                    });
                    f.stroke(&v, Stroke::default().with_color(TEXTO).with_width((1.6 * z).max(1.0)));
                }
            } else {
                f.fill(&Path::circle(q.center(), lado / 2.0), BRANCO);
                f.stroke(&Path::circle(q.center(), lado / 2.0), Stroke::default().with_color(BORDA).with_width(z.max(0.5)));
                if ct.marcado { f.fill(&Path::circle(q.center(), lado * 0.22), TEXTO); }
            }
            texto_l(f, letra, rotulo, Point::new(q.x + lado + 4.0 * z, base.center_y()), tam, pelicano::Align::Start, cor_txt);
        }
        T::Girar => {
            let vertical = base.height >= base.width;
            let (a, b) = if vertical {
                (Rectangle { height: base.height / 2.0, ..base }, Rectangle { y: base.y + base.height / 2.0, height: base.height / 2.0, ..base })
            } else {
                (Rectangle { width: base.width / 2.0, ..base }, Rectangle { x: base.x + base.width / 2.0, width: base.width / 2.0, ..base })
            };
            caixa(f, a, FACE, z);
            caixa(f, b, FACE, z);
            seta(f, a, if vertical { '^' } else { '<' });
            seta(f, b, if vertical { 'v' } else { '>' });
        }
        T::Rolagem => {
            caixa(f, base, Color::rgb(0.9, 0.9, 0.9), z);
            let (comp, grosso) = if ct.horizontal { (base.width, base.height) } else { (base.height, base.width) };
            let s = grosso.min(comp / 3.0);
            let fracao = ((ct.valor - ct.min) as f32 / (ct.max - ct.min).max(1) as f32).clamp(0.0, 1.0);
            let pol = s + fracao * (comp - 3.0 * s);
            let (a, b, p) = if ct.horizontal {
                (Rectangle { width: s, ..base }, Rectangle { x: base.x + comp - s, width: s, ..base }, Rectangle { x: base.x + pol, width: s, ..base })
            } else {
                (Rectangle { height: s, ..base }, Rectangle { y: base.y + comp - s, height: s, ..base }, Rectangle { y: base.y + pol, height: s, ..base })
            };
            for q in [a, b, p] { caixa(f, q, FACE, z); }
            seta(f, a, if ct.horizontal { '<' } else { '^' });
            seta(f, b, if ct.horizontal { '>' } else { 'v' });
        }
        T::Lista => {
            caixa(f, base, BRANCO, z);
            lista(f, &ct.itens, ct.escolhido, base, tam, z);
        }
        T::Combo => {
            caixa(f, base, BRANCO, z);
            let botao = Rectangle { x: base.x + base.width - base.height.min(17.0 * z), width: base.height.min(17.0 * z), ..base };
            caixa(f, botao, FACE, z);
            seta(f, botao, 'v');
            if let Some(t) = ct.escolhido.and_then(|i| ct.itens.get(i)) {
                texto(f, t, Point::new(base.x + 3.0 * z, base.center_y()), tam, pelicano::Align::Start, TEXTO);
            }
            if ct.aberto {
                let abaixo = Rectangle { y: base.y + base.height, height: r.height - base.height, ..base };
                caixa(f, abaixo, BRANCO, z);
                lista(f, &ct.itens, ct.escolhido, abaixo, tam, z);
            }
        }
    }
}

fn lista(f: &mut Frame, itens: &[String], escolhido: Option<usize>, r: Rectangle, tam: f32, z: f32) {
    let alto = LINHA_LISTA * z;
    for (i, t) in itens.iter().enumerate().take((r.height / alto).ceil() as usize) {
        let y = r.y + i as f32 * alto;
        let cor = if escolhido == Some(i) {
            f.fill_rectangle(Point::new(r.x + z, y), Size::new(r.width - 2.0 * z, alto), ESCOLHA);
            BRANCO
        } else { TEXTO };
        texto(f, t, Point::new(r.x + 3.0 * z, y + alto / 2.0), tam, pelicano::Align::Start, cor);
    }
}

#[cfg(test)]
mod testes_clique {
    use super::*;
    use feathersheet_io::objetos_aba::{Ancora, ObjetoSolto, ObjetosAba};

    #[test]
    fn arrastar_a_caixa_da_barra_muda_o_valor() {
        let mut c = ControleFormulario::novo(T::Rolagem);
        (c.min, c.max, c.horizontal) = (Some(0), Some(100), true);
        // Barra deitada de 130 × 20: setas de 20, curso de 70; valor 50 = caixa em 55..75.
        assert_eq!(valor_arrastado(&c, Some(50), (130.0, 20.0), (60.0, 10.0), (95.0, 10.0)), Some(100));
        assert_eq!(valor_arrastado(&c, Some(50), (130.0, 20.0), (60.0, 10.0), (25.0, 10.0)), Some(0));
        assert_eq!(valor_arrastado(&c, Some(50), (130.0, 20.0), (60.0, 10.0), (67.0, 10.0)), Some(60));
        // Começou na seta, não na caixa: não é arrasto.
        assert_eq!(valor_arrastado(&c, Some(50), (130.0, 20.0), (5.0, 10.0), (95.0, 10.0)), None);
    }

    #[test]
    fn clicar_na_caixa_de_listagem_escolhe_o_item() {
        let mut app = App::default();
        for (r, t) in ["Caneta", "Caderno", "Mochila", "Estojo"].iter().enumerate() {
            app.workbook.escrever_celula(0, 1 + r as u32, 6, (*t).into());
        }
        let mut c = ControleFormulario::novo(T::Lista);
        (c.celula, c.faixa, c.valor) = (Some("$C$8".into()), Some("$G$2:$G$5".into()), Some(2));
        let mut lista = ObjetosAba::default();
        let id = lista.adicionar(ObjetoSolto::novo_controle(c, Ancora { linha: 7, coluna: 1, dx: 4.0, dy: 2.0 }, Ancora { linha: 11, coluna: 1, dx: 134.0, dy: 0.0 })).unwrap();
        app.workbook.aba_mut(0).unwrap().sheet.ob = lista.gravar();
        app.clicar_controle(id, 10.0, LINHA_LISTA * 2.5);
        assert_eq!(app.workbook.valor(0, 7, 2), feathersheet_core::Value::Number(3.0), "{}", app.status);
    }
}

