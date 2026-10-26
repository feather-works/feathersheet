//! Desenho dos formulários das macros no Alt+F11 (o designer de UserForm do
//! VBA): escolher o formulário, pôr controles pela caixa de ferramentas,
//! ajustar as propriedades do escolhido, guardar e testar. A edição é numa
//! cópia; "Guardar" valida e grava em `Workbook::formularios`.
use super::*;
use super::editor_visual::estilos::*;
use crate::compat::button;
use feathersheet_macros::formularios::{self, Controle, Formulario, TipoControle};
use pelicano::{checkbox, column, container, row, scrollable, text, text_input, Element, Length, Space};

#[derive(Debug, Default, Clone)]
pub(in crate::app) struct Designer {
    pub form: Option<Formulario>,
    /// O nome com que o formulário está guardado (None = novo, ainda não guardado).
    pub original: Option<String>,
    pub sel: Option<usize>,
    pub aviso: String,
    /// Arrasto em curso: controle, se é a alça (redimensionar) e, depois do
    /// primeiro movimento, o ponto inicial e o retângulo inicial.
    pub arrasto: Option<(usize, bool, Option<((f32, f32), [f32; 4])>)>,
    /// Arrastando uma moldura: os controles de dentro e a posição de partida deles.
    pub filhos: Vec<(usize, f32, f32)>,
    /// O campo de cor com a paleta aberta.
    pub cor_aberta: Option<Campo>,
}

use crate::app::formulario_macro::aparencia as ap;

/// A paleta das amostras de cor (o resto, pelo `#rrggbb`).
const PALETA: [&str; 24] = ["#000000", "#434343", "#666666", "#999999", "#cccccc", "#ffffff", "#e53935", "#fb8c00",
    "#fdd835", "#43a047", "#00897b", "#1e88e5", "#3949ab", "#8e24aa", "#d81b60", "#6d4c41",
    "#ffebee", "#fff3e0", "#fffde7", "#e8f5e9", "#e3f2fd", "#ede7f6", "#fce4ec", "#efebe9"];

/// Imagem acima disto é reduzida (lado maior 1024 px, JPEG) antes de entrar no arquivo.
const IMAGEM_GRANDE: usize = 1_400_000;

/// O designer do VBA encaixa numa grade; aqui, 4 px.
const GRADE: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo { FormNome, FormTitulo, FormLargura, FormAltura, FormFundo, Nome, Texto, X, Y, Largura, Altura, Opcoes, Fonte, Valor, Macro,
    Fundo, Cor, Borda, Raio, Letra, Familia, Alinhar, Max, Dica }

/// As propriedades liga/desliga do controle escolhido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marca { Limpar, Fechar, Padrao, Cancelar, Desativado, Oculto, Livre, Multipla, Negrito, Italico, Multilinha }

#[derive(Debug, Clone)]
pub enum EventoDesigner {
    Abrir(String),
    Novo,
    Adicionar(TipoControle),
    Selecionar(Option<usize>),
    /// Apertou no controle (false) ou na alça do canto (true).
    ArrastoInicio(usize, bool),
    ArrastoMover(f32, f32),
    ArrastoFim,
    Campo(Campo, String),
    Marcar(Marca, bool),
    RemoverControle,
    /// Abre (ou fecha, com `None`) a paleta de um campo de cor.
    AbrirCor(Option<Campo>),
    EscolherImagem,
    ImagemCarregada(Result<Option<Vec<u8>>, String>),
    RemoverImagem,
    Guardar,
    Testar,
    Excluir,
}

fn rotulo_tipo(t: TipoControle) -> &'static str {
    match t {
        TipoControle::Rotulo => tr!("Rótulo"), TipoControle::Texto => tr!("Campo de texto"),
        TipoControle::Numero => tr!("Campo numérico"), TipoControle::Senha => tr!("Senha"),
        TipoControle::Caixa => tr!("Caixa de seleção"), TipoControle::Opcao => tr!("Opção"),
        TipoControle::Lista => tr!("Lista suspensa"), TipoControle::Botao => tr!("Botão"),
        TipoControle::Intervalo => tr!("Intervalo (RefEdit)"), TipoControle::Listagem => tr!("Caixa de listagem"),
        TipoControle::Moldura => tr!("Moldura"), TipoControle::Imagem => tr!("Imagem"),
    }
}

fn icone_tipo(t: TipoControle) -> &'static str {
    match t {
        TipoControle::Rotulo => "text-size", TipoControle::Texto => "input", TipoControle::Numero => "sum",
        TipoControle::Senha => "lock", TipoControle::Caixa => "checkbox", TipoControle::Opcao => "circle-dot",
        TipoControle::Lista => "list-select", TipoControle::Botao => "square", TipoControle::Intervalo => "table",
        TipoControle::Listagem => "list", TipoControle::Moldura => "border-all", TipoControle::Imagem => "photo",
    }
}

fn numero(v: f32) -> String { if v.fract() == 0.0 { format!("{v:.0}") } else { v.to_string() } }

fn ler_numero(t: &str) -> f32 { t.trim().replace(',', ".").parse::<f32>().unwrap_or(0.0).clamp(0.0, 2000.0) }

/// Vazio = segue o padrão.
fn ler_opcional(t: &str, min: f32, max: f32) -> Option<f32> { t.trim().replace(',', ".").parse::<f32>().ok().map(|v| v.clamp(min, max)) }

/// Um nome livre para o controle novo ("texto1", "texto2"…).
fn nome_livre(form: &Formulario, tipo: TipoControle) -> String {
    (1..).map(|n| format!("{}{n}", tipo.nome())).find(|c| !form.controles.iter().any(|k| &k.nome == c)).expect("sempre há nome livre")
}

impl App {
    pub(in crate::app) fn evento_designer(&mut self, evento: EventoDesigner) {
        if matches!(evento, EventoDesigner::Abrir(_) | EventoDesigner::Novo) {
            self.macro_visual.documento = "componente:previsao".into();
        }
        let d = &mut self.macro_visual.designer;
        match evento {
            EventoDesigner::Abrir(nome) => {
                match formularios::ler(self.workbook.formularios.as_deref()) {
                    Ok(lista) => if let Some(f) = formularios::obter(&lista, &nome) {
                        *d = Designer { original: Some(f.nome.clone()), form: Some(f.clone()), ..Default::default() };
                    },
                    Err(e) => d.aviso = e,
                }
            }
            EventoDesigner::Novo => {
                let lista = formularios::ler(self.workbook.formularios.as_deref()).unwrap_or_default();
                let nome = (1..).map(|n| format!("Formulario{n}")).find(|c| formularios::obter(&lista, c).is_none()).expect("sempre há nome livre");
                *d = Designer { form: Some(Formulario::novo(&nome)), ..Default::default() };
            }
            EventoDesigner::Adicionar(tipo) => if let Some(f) = d.form.as_mut() {
                // Abaixo do último controle, na margem esquerda.
                let y = f.controles.iter().map(|c| c.y + c.altura).fold(0.0f32, f32::max) + 8.0;
                let (largura, altura) = match tipo { TipoControle::Botao => (110.0, 30.0), TipoControle::Rotulo => (160.0, 20.0),
                    TipoControle::Listagem => (200.0, 96.0), TipoControle::Moldura => (240.0, 120.0), TipoControle::Imagem => (120.0, 90.0),
                    _ => (200.0, 26.0) };
                let texto = match tipo { TipoControle::Rotulo => tr!("Rótulo"), TipoControle::Botao => tr!("OK"), TipoControle::Moldura => tr!("Moldura"),
                    TipoControle::Caixa | TipoControle::Opcao => rotulo_tipo(tipo), _ => "" };
                let nome = if tipo.tem_valor() { nome_livre(f, tipo) } else { String::new() };
                f.controles.push(Controle { tipo, nome, texto: texto.into(), x: 12.0, y: y.min((f.altura - altura).max(0.0)),
                    largura, altura, ..Default::default() });
                d.sel = Some(f.controles.len() - 1);
            },
            EventoDesigner::Selecionar(i) => d.sel = i,
            EventoDesigner::ArrastoInicio(i, alca) => {
                d.sel = Some(i);
                d.arrasto = Some((i, alca, None));
                // A moldura leva junto o que está inteiro dentro dela, como o Frame do VBA.
                d.filhos = match d.form.as_ref().and_then(|f| f.controles.get(i)).filter(|m| !alca && m.tipo == TipoControle::Moldura) {
                    Some(m) => d.form.as_ref().map_or(Vec::new(), |f| f.controles.iter().enumerate()
                        .filter(|(j, c)| *j != i && c.x >= m.x && c.y >= m.y && c.x + c.largura <= m.x + m.largura && c.y + c.altura <= m.y + m.altura)
                        .map(|(j, c)| (j, c.x, c.y)).collect()),
                    None => Vec::new(),
                };
            }
            EventoDesigner::ArrastoFim => { d.arrasto = None; d.filhos.clear(); }
            EventoDesigner::ArrastoMover(px, py) => if let (Some((i, alca, inicio)), Some(f)) = (d.arrasto.as_mut(), d.form.as_mut()) {
                let (largura_form, altura_form) = (f.largura, f.altura);
                let Some(c) = f.controles.get_mut(*i) else { return };
                let ((x0, y0), [cx, cy, cl, ca]) = *inicio.get_or_insert(((px, py), [c.x, c.y, c.largura, c.altura]));
                let encaixar = |v: f32| (v / GRADE).round() * GRADE;
                let (dx, dy) = (px - x0, py - y0);
                if *alca {
                    c.largura = encaixar(cl + dx).clamp(8.0, (largura_form - c.x).max(8.0));
                    c.altura = encaixar(ca + dy).clamp(8.0, (altura_form - c.y).max(8.0));
                } else {
                    c.x = encaixar(cx + dx).clamp(0.0, (largura_form - c.largura).max(0.0));
                    c.y = encaixar(cy + dy).clamp(0.0, (altura_form - c.altura).max(0.0));
                    let (ddx, ddy) = (c.x - cx, c.y - cy);
                    for &(j, x, y) in &d.filhos {
                        if let Some(k) = f.controles.get_mut(j) { (k.x, k.y) = ((x + ddx).max(0.0), (y + ddy).max(0.0)); }
                    }
                }
            },
            EventoDesigner::AbrirCor(c) => d.cor_aberta = c,
            EventoDesigner::EscolherImagem | EventoDesigner::ImagemCarregada(Ok(None)) => {}
            EventoDesigner::ImagemCarregada(Err(e)) => d.aviso = e,
            EventoDesigner::ImagemCarregada(Ok(Some(bytes))) => match imagem_para_guardar(&bytes) {
                Ok(b64) => if let Some(c) = d.sel.and_then(|i| d.form.as_mut()?.controles.get_mut(i)) { c.imagem = b64; },
                Err(e) => d.aviso = e,
            },
            EventoDesigner::RemoverImagem => if let Some(c) = d.sel.and_then(|i| d.form.as_mut()?.controles.get_mut(i)) { c.imagem.clear(); },
            EventoDesigner::Campo(campo, t) => if let Some(f) = d.form.as_mut() {
                match campo {
                    Campo::FormNome => f.nome = t,
                    Campo::FormTitulo => f.titulo = t,
                    Campo::FormLargura => f.largura = ler_numero(&t),
                    Campo::FormAltura => f.altura = ler_numero(&t),
                    Campo::FormFundo => { f.fundo = t; d.cor_aberta = None; }
                    _ => if let Some(c) = d.sel.and_then(|i| f.controles.get_mut(i)) {
                        match campo {
                            Campo::Nome => c.nome = t,
                            Campo::Texto => c.texto = t,
                            Campo::X => c.x = ler_numero(&t),
                            Campo::Y => c.y = ler_numero(&t),
                            Campo::Largura => c.largura = ler_numero(&t),
                            Campo::Altura => c.altura = ler_numero(&t),
                            Campo::Opcoes => c.opcoes = t.split(';').map(|o| o.trim().to_owned()).filter(|o| !o.is_empty()).collect(),
                            Campo::Fonte => c.fonte = t,
                            Campo::Valor => c.valor = t,
                            Campo::Macro => c.macro_nome = Some(t).filter(|m| !m.is_empty() && m != tr!("(nenhuma)")),
                            Campo::Fundo => { c.fundo = t; d.cor_aberta = None; }
                            Campo::Cor => { c.cor = t; d.cor_aberta = None; }
                            Campo::Borda => { c.borda = t; d.cor_aberta = None; }
                            Campo::Raio => c.raio = ler_opcional(&t, 0.0, 200.0),
                            Campo::Letra => c.letra = ler_opcional(&t, 6.0, 96.0),
                            Campo::Familia => c.familia = if t == tr!("Serifa") { "serifa".into() } else if t == tr!("Monoespaçada") { "mono".into() } else { String::new() },
                            Campo::Alinhar => c.alinhar = t,
                            Campo::Max => c.max = t.trim().parse().unwrap_or(0),
                            Campo::Dica => c.dica = t,
                            _ => {}
                        }
                    },
                }
            },
            EventoDesigner::Marcar(m, b) => if let (Some(f), Some(i)) = (d.form.as_mut(), d.sel) {
                // Um só botão padrão e um só de cancelar, como no VBA.
                if b && matches!(m, Marca::Padrao | Marca::Cancelar) {
                    for c in f.controles.iter_mut() { if m == Marca::Padrao { c.padrao = false } else { c.cancelar = false } }
                }
                if let Some(c) = f.controles.get_mut(i) {
                    *match m { Marca::Limpar => &mut c.limpar, Marca::Fechar => &mut c.fechar, Marca::Padrao => &mut c.padrao,
                        Marca::Cancelar => &mut c.cancelar, Marca::Desativado => &mut c.desativado, Marca::Oculto => &mut c.oculto,
                        Marca::Livre => &mut c.livre, Marca::Multipla => &mut c.multipla, Marca::Negrito => &mut c.negrito,
                        Marca::Italico => &mut c.italico, Marca::Multilinha => &mut c.multilinha } = b;
                }
            },
            EventoDesigner::RemoverControle => if let (Some(f), Some(i)) = (d.form.as_mut(), d.sel) {
                if i < f.controles.len() { f.controles.remove(i); }
                d.sel = None;
            },
            EventoDesigner::Guardar => { self.guardar_formulario(); }
            EventoDesigner::Testar => if self.guardar_formulario() {
                let nome = self.macro_visual.designer.form.as_ref().map(|f| f.nome.clone()).unwrap_or_default();
                self.abrir_formulario_macro(&nome, &serde_json::Value::Null);
            },
            EventoDesigner::Excluir => {
                let Some(original) = d.original.clone() else { *d = Designer::default(); return };
                let mut lista = formularios::ler(self.workbook.formularios.as_deref()).unwrap_or_default();
                lista.retain(|f| !f.nome.eq_ignore_ascii_case(&original));
                match formularios::gravar(&lista) {
                    Ok(texto) => { self.workbook.formularios = texto; self.alterado = true; self.macro_visual.designer = Designer::default(); }
                    Err(e) => self.macro_visual.designer.aviso = e,
                }
            }
        }
    }

    /// Valida e grava a cópia em edição; o aviso diz o que impediu.
    fn guardar_formulario(&mut self) -> bool {
        let d = &mut self.macro_visual.designer;
        let Some(form) = d.form.clone() else { return false };
        let mut lista = match formularios::ler(self.workbook.formularios.as_deref()) { Ok(l) => l, Err(e) => { d.aviso = e; return false } };
        if let Some(m) = form.controles.iter().filter_map(|c| c.macro_nome.as_deref()).find(|m| self.macro_catalogo.obter(m).is_none()) {
            d.aviso = trf!("Não existe a macro \"{m}\".");
            return false;
        }
        let alvo = d.original.as_deref().unwrap_or(&form.nome).to_owned();
        match lista.iter_mut().find(|f| f.nome.eq_ignore_ascii_case(&alvo)) {
            Some(f) => *f = form.clone(),
            None => lista.push(form.clone()),
        }
        match formularios::gravar(&lista) {
            Ok(texto) => {
                self.workbook.formularios = texto;
                self.alterado = true;
                d.original = Some(form.nome.clone());
                d.aviso = trf!("Formulário \"{}\" guardado. Abra pela macro com sheet.showComponent(\"{}\").", form.nome, form.nome);
                true
            }
            Err(e) => { d.aviso = e; false }
        }
    }
}

fn ev(e: EventoDesigner) -> Msg { Msg::MacroVisual(EventoVisual::Designer(e)) }

/// A imagem em base64 para o controle; foto grande é reduzida antes.
fn imagem_para_guardar(bytes: &[u8]) -> Result<String, String> {
    use base64::Engine;
    let img = image::load_from_memory(bytes).map_err(|e| trf!("Não consegui abrir a imagem: {e}"))?;
    let bytes = if bytes.len() <= IMAGEM_GRANDE { bytes.to_vec() } else {
        let mut saida = std::io::Cursor::new(Vec::new());
        img.thumbnail(1024, 1024).to_rgb8().write_to(&mut saida, image::ImageFormat::Jpeg).map_err(|e| trf!("Não consegui reduzir a imagem: {e}"))?;
        saida.into_inner()
    };
    if bytes.len() > IMAGEM_GRANDE { return Err(tr!("A imagem continua grande demais mesmo reduzida.").into()); }
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

impl App {
    /// "Escolher imagem…" do controle Imagem: o seletor de arquivo do sistema.
    pub(in crate::app) fn escolher_imagem_designer(&mut self) -> Task<Msg> {
        Task::perform(async {
            let Some(arquivo) = rfd::AsyncFileDialog::new().set_directory(crate::app::dialogos::pasta_padrao())
                .add_filter(tr!("Imagens"), &["png", "jpg", "jpeg", "gif", "bmp", "webp"])
                .pick_file().await else { return Ok(None) };
            let caminho = arquivo.path().to_path_buf();
            tokio::task::spawn_blocking(move || std::fs::read(&caminho).map(Some).map_err(|e| trf!("Falha ao ler a imagem: {e}")))
                .await.unwrap_or_else(|e| Err(trf!("Falha ao ler a imagem: {e}")))
        }, |r| ev(EventoDesigner::ImagemCarregada(r)))
    }
}

fn campo<'a>(rotulo: &str, valor: String, c: Campo) -> Element<'a, Msg> {
    row![
        text(rotulo.to_owned()).size(11).width(Length::Fixed(78.0)),
        text_input("", valor).on_input(move |t| ev(EventoDesigner::Campo(c, t))).size(11).padding([4, 6]),
    ].spacing(6).align(pelicano::Align::Center).into()
}

/// A prévia de um controle no desenho (sem interação: o clique escolhe), com a aparência gravada.
fn previa<'a>(c: &'a Controle, escuro: bool, escolhido: bool, fundo_form: pelicano::Color) -> Element<'a, Msg> {
    let (tam, fonte) = (ap::letra(c), ap::fonte(c));
    let marcar = move |mut st: pelicano::Style| { if escolhido { st.border.color = cor_acento(); st.border.width = 2.0; } st };
    let tinta = ap::cor(&c.cor);
    match c.tipo {
        TipoControle::Moldura => {
            let borda = ap::cor(&c.borda).unwrap_or(cor_borda(escuro));
            let (raio, fundo) = (c.raio.unwrap_or(4.0), ap::cor(&c.fundo));
            let caixa = container(Space::new(Length::FILL, Length::FILL)).width(Length::FILL).height(Length::FILL)
                .style(move |_| marcar(pelicano::Style { background: fundo.map(pelicano::Background::Color),
                    border: pelicano::Border { color: borda, width: 1.0, radius: [raio; 4] }, ..Default::default() }));
            let mut camadas: Vec<Element<'a, Msg>> = vec![column![Space::new(0.0, Length::Fixed(tam * 0.7)), caixa].into()];
            if !c.texto.is_empty() {
                let atras = fundo.unwrap_or(fundo_form);
                camadas.push(row![Space::new(Length::Fixed(10.0), 0.0),
                    container(text(c.texto.clone()).size(tam).font(fonte).color(tinta.unwrap_or(cor_texto(escuro)))).padding([0, 4])
                        .style(move |_| pelicano::Style { background: Some(pelicano::Background::Color(atras)), ..Default::default() })].into());
            }
            return pelicano::stack(camadas).width(Length::FILL).height(Length::FILL).into();
        }
        TipoControle::Imagem => {
            let corpo: Element<'a, Msg> = match ap::imagem(&c.imagem) {
                Some(img) => { let mut v = pelicano::image(img).width(Length::FILL).height(Length::FILL); if let Some(r) = c.raio { v = v.radius(r); } v.into() }
                None => container(column![feather_ui::icones::icone("photo", 22.0), text(tr!("Imagem")).size(11).color(cor_secundaria(escuro))]
                    .spacing(4).align(pelicano::Align::Center)).width(Length::FILL).height(Length::FILL)
                    .align_x(pelicano::Align::Center).align_y(pelicano::Align::Center).into(),
            };
            let vazia = c.imagem.is_empty();
            return container(corpo).width(Length::FILL).height(Length::FILL)
                .style(move |_| marcar(ap::sobre(c, pelicano::Style { border: pelicano::Border { color: cor_borda(escuro),
                    width: if vazia { 1.0 } else { 0.0 }, radius: [3.0; 4] }, ..Default::default() }))).into();
        }
        _ => {}
    }
    let corpo = match c.tipo {
        TipoControle::Rotulo | TipoControle::Botao | TipoControle::Moldura | TipoControle::Imagem => c.texto.clone(),
        TipoControle::Texto | TipoControle::Numero | TipoControle::Senha => if c.texto.is_empty() { c.nome.clone() } else { c.texto.clone() },
        TipoControle::Intervalo => format!("{}  ⊞", if c.texto.is_empty() { &c.nome } else { &c.texto }),
        TipoControle::Caixa => format!("☐ {}", c.texto),
        TipoControle::Opcao => format!("◯ {}", c.texto),
        TipoControle::Lista => format!("{} ▾", c.opcoes.first().cloned().unwrap_or_else(|| c.nome.clone())),
        TipoControle::Listagem => if c.opcoes.is_empty() { c.nome.clone() } else { c.opcoes.join("\n") },
    };
    let caixa = matches!(c.tipo, TipoControle::Texto | TipoControle::Numero | TipoControle::Senha | TipoControle::Lista | TipoControle::Listagem | TipoControle::Intervalo | TipoControle::Botao);
    let botao = c.tipo == TipoControle::Botao;
    let em_cima = c.tipo == TipoControle::Listagem || c.multilinha;
    container(text(corpo).size(tam).font(fonte).color(tinta.unwrap_or(if caixa && !botao { cor_secundaria(escuro) } else { cor_texto(escuro) })))
        .padding([if em_cima { 3 } else { 0 }, 6]).width(Length::FILL).height(Length::FILL)
        .align_y(if em_cima { pelicano::Align::Start } else { pelicano::Align::Center })
        .align_x(ap::alinhamento(c))
        .style(move |_| marcar(ap::sobre(c, pelicano::Style {
            background: caixa.then(|| pelicano::Background::Color(if botao { cor_chrome(escuro) } else { cor_fundo(escuro) })),
            border: pelicano::Border { color: cor_borda(escuro), width: if caixa { 1.0 } else { 0.0 }, radius: [3.0; 4] },
            ..Default::default()
        })))
        .into()
}

/// Uma cor do controle: amostra (abre a paleta), `#rrggbb` e "sem cor" (volta ao tema).
fn linha_cor<'a>(rotulo: &str, valor: &str, campo: Campo, aberta: bool) -> Element<'a, Msg> {
    let cor = ap::cor(valor);
    let amostra = button(Space::new(Length::Fixed(18.0), Length::Fixed(18.0))).padding(0)
        .style(move |_, _| pelicano::Style {
            background: cor.map(pelicano::Background::Color),
            border: pelicano::Border { color: COR_BORDA(), width: 1.0, radius: [4.0; 4] },
            ..Default::default()
        })
        .on_press(ev(EventoDesigner::AbrirCor((!aberta).then_some(campo))));
    let mut linha = row![
        text(rotulo.to_owned()).size(11).width(Length::Fixed(78.0)),
        amostra,
        text_input(tr!("tema"), valor.to_owned()).on_input(move |t| ev(EventoDesigner::Campo(campo, t))).size(11).padding([4, 6]),
    ].spacing(6).align(pelicano::Align::Center);
    if !valor.is_empty() {
        linha = linha.push(pelicano::tooltip(button(feather_ui::icones::icone("x", 12.0)).padding([3, 4]).style(estilo_botao_secundario)
            .on_press(ev(EventoDesigner::Campo(campo, String::new()))), tr!("Sem cor (tema)"), pelicano::TooltipPosition::Top));
    }
    if !aberta { return linha.into(); }
    let mut grade = column![].spacing(4);
    for fileira in PALETA.chunks(8) {
        let mut l = row![].spacing(4);
        for &hex in fileira {
            let k = ap::cor(hex);
            let escolhida = valor.eq_ignore_ascii_case(hex);
            l = l.push(button(Space::new(Length::Fixed(16.0), Length::Fixed(16.0))).padding(0)
                .style(move |_, st| pelicano::Style {
                    background: k.map(pelicano::Background::Color),
                    border: pelicano::Border { color: if escolhida || matches!(st, pelicano::Status::Hovered) { COR_AZUL_ACENTO() } else { COR_BORDA() },
                        width: if escolhida { 2.0 } else { 1.0 }, radius: [4.0; 4] },
                    ..Default::default()
                })
                .on_press(ev(EventoDesigner::Campo(campo, hex.to_owned()))));
        }
        grade = grade.push(l);
    }
    column![linha, row![Space::new(Length::Fixed(84.0), 0.0), grade]].spacing(6).into()
}

/// Botão liga/desliga com ícone (negrito, itálico, alinhamento).
fn botao_marca<'a>(icone: &str, dica: &str, ativo: bool, m: Msg) -> Element<'a, Msg> {
    pelicano::tooltip(button(feather_ui::icones::icone(icone, 14.0)).padding([4, 6]).style(feather_ui::tema::estilo_botao_pequeno(ativo)).on_press(m),
        dica.to_owned(), pelicano::TooltipPosition::Top).into()
}

pub(super) fn designer<'a>(app: &'a App) -> Element<'a, Msg> {
    let escuro = app.config.escuro;
    let d = &app.macro_visual.designer;
    let nomes: Vec<String> = formularios::ler(app.workbook.formularios.as_deref()).unwrap_or_default().into_iter().map(|f| f.nome).collect();
    let tem = d.form.is_some();
    let barra = row![
        crate::compat::pick_list_vec(nomes, d.original.clone(), |n| ev(EventoDesigner::Abrir(n)))
            .width(Length::Fixed(200.0)),
        botao_icone(tr!("Novo"), "plus", Some(ev(EventoDesigner::Novo)), false),
        Space::new(Length::FILL, 0.0),
        botao_icone(tr!("Guardar"), "device-floppy", tem.then(|| ev(EventoDesigner::Guardar)), false),
        botao_icone(tr!("Testar"), "player-play", tem.then(|| ev(EventoDesigner::Testar)), false),
        botao_icone(tr!("Excluir formulário"), "trash", d.original.is_some().then(|| ev(EventoDesigner::Excluir)), false),
    ].spacing(4).padding([4, 7]).align(pelicano::Align::Center);
    let Some(f) = &d.form else {
        return column![
            container(barra).style(estilo_cabecalho(escuro)),
            container(text(tr!("Escolha um formulário ou crie um novo. A macro abre o formulário com sheet.showComponent(\"nome\"); o botão roda a macro dele, que lê o preenchido em valores.")).size(12))
                .padding(24).width(Length::FILL).height(Length::FILL),
        ].height(Length::FILL).into();
    };
    let mut caixa = column![titulo(tr!("CAIXA DE FERRAMENTAS"), escuro)].spacing(5).padding(8);
    for t in TipoControle::TODOS {
        caixa = caixa.push(botao_icone(rotulo_tipo(t), icone_tipo(t), Some(ev(EventoDesigner::Adicionar(t))), false));
    }
    // O formulário no tamanho real; cada controle é escolhido pelo clique. Molduras atrás, como na janela.
    let fundo_form = ap::cor(&f.fundo).unwrap_or(cor_fundo(escuro));
    let mut ordem: Vec<usize> = (0..f.controles.len()).collect();
    ordem.sort_by_key(|&i| f.controles[i].tipo != TipoControle::Moldura);
    let camadas: Vec<Element<'a, Msg>> = std::iter::once(
        pelicano::mouse_area(Space::new(Length::FILL, Length::FILL)).on_press(ev(EventoDesigner::Selecionar(None))).into()
    ).chain(ordem.into_iter().map(|i| {
        let c = &f.controles[i];
        let escolhido = d.sel == Some(i);
        let alvo = pelicano::mouse_area(container(previa(c, escuro, escolhido, fundo_form)).width(Length::Fixed(c.largura)).height(Length::Fixed(c.altura)))
            .cursor_icon(crate::compat::mouse::Interaction::Grab)
            .on_press(ev(EventoDesigner::ArrastoInicio(i, false)));
        let mut camada = pelicano::stack(vec![column![
            Space::new(0.0, Length::Fixed(c.y)),
            row![Space::new(Length::Fixed(c.x), 0.0), alvo],
        ].into()]);
        if escolhido {
            // A alça de redimensionar no canto de baixo à direita.
            let alca = pelicano::mouse_area(container(Space::new(Length::Fixed(8.0), Length::Fixed(8.0)))
                .style(|_| pelicano::Style { background: Some(pelicano::Background::Color(cor_acento())), ..Default::default() }))
                .cursor_icon(crate::compat::mouse::Interaction::NwseResize)
                .on_press(ev(EventoDesigner::ArrastoInicio(i, true)));
            camada = camada.push(column![
                Space::new(0.0, Length::Fixed((c.y + c.altura - 5.0).max(0.0))),
                row![Space::new(Length::Fixed((c.x + c.largura - 5.0).max(0.0)), 0.0), alca],
            ]);
        }
        camada.into()
    })).collect();
    let folha = container(pelicano::stack(camadas).width(Length::Fixed(f.largura)).height(Length::Fixed(f.altura)))
        .padding(16).style(move |t| pelicano::Style { background: Some(pelicano::Background::Color(fundo_form)), ..estilo_area(escuro)(t) });
    let desenho = scrollable(column![
        text(if f.titulo.is_empty() { f.nome.clone() } else { f.titulo.clone() }).size(13).color(cor_secundaria(escuro)),
        folha,
    ].spacing(6).padding(18)).direction(pelicano::Direction::Both)
        .width(Length::FILL).height(Length::FILL);
    let mut props = column![
        titulo(tr!("FORMULÁRIO"), escuro),
        campo(tr!("Nome"), f.nome.clone(), Campo::FormNome),
        campo(tr!("Título"), f.titulo.clone(), Campo::FormTitulo),
        campo(tr!("Largura"), numero(f.largura), Campo::FormLargura),
        campo(tr!("Altura"), numero(f.altura), Campo::FormAltura),
        linha_cor(tr!("Fundo"), &f.fundo, Campo::FormFundo, d.cor_aberta == Some(Campo::FormFundo)),
    ].spacing(5).padding(8);
    if let Some(c) = d.sel.and_then(|i| f.controles.get(i)) {
        props = props.push(titulo(&rotulo_tipo(c.tipo).to_uppercase(), escuro));
        if c.tipo.tem_valor() { props = props.push(campo(tr!("Nome"), c.nome.clone(), Campo::Nome)); }
        let rotulo_texto = match c.tipo {
            TipoControle::Texto | TipoControle::Numero | TipoControle::Senha | TipoControle::Intervalo => tr!("Exemplo"),
            TipoControle::Moldura => tr!("Título"),
            _ => tr!("Texto"),
        };
        if !matches!(c.tipo, TipoControle::Lista | TipoControle::Listagem | TipoControle::Imagem) { props = props.push(campo(rotulo_texto, c.texto.clone(), Campo::Texto)); }
        for (r, v, k) in [(tr!("X"), c.x, Campo::X), (tr!("Y"), c.y, Campo::Y), (tr!("Largura"), c.largura, Campo::Largura), (tr!("Altura"), c.altura, Campo::Altura)] {
            props = props.push(campo(r, numero(v), k));
        }
        if matches!(c.tipo, TipoControle::Lista | TipoControle::Listagem) {
            props = props.push(campo(tr!("Itens (;)"), c.opcoes.join("; "), Campo::Opcoes));
            props = props.push(campo(tr!("Itens de"), c.fonte.clone(), Campo::Fonte));
        }
        let marca = |rotulo: &str, valor: bool, m: Marca| -> Element<'a, Msg> {
            checkbox(rotulo.to_owned(), valor).on_toggle(move |b| ev(EventoDesigner::Marcar(m, b))).size(12).into()
        };
        if c.tipo == TipoControle::Lista { props = props.push(marca(tr!("Aceita digitar"), c.livre, Marca::Livre)); }
        if c.tipo == TipoControle::Listagem { props = props.push(marca(tr!("Várias escolhas"), c.multipla, Marca::Multipla)); }
        if c.tipo.tem_valor() {
            let dica = match c.tipo {
                TipoControle::Caixa | TipoControle::Opcao => tr!("Marcado (1)"),
                TipoControle::Listagem if c.multipla => tr!("Marcados (;)"),
                _ => tr!("Valor inicial"),
            };
            props = props.push(campo(dica, c.valor.clone(), Campo::Valor));
        }
        if c.tipo == TipoControle::Texto {
            props = props.push(campo(tr!("Máx. letras"), if c.max == 0 { String::new() } else { c.max.to_string() }, Campo::Max));
            props = props.push(marca(tr!("Várias linhas"), c.multilinha, Marca::Multilinha));
        }
        if c.tipo == TipoControle::Imagem {
            props = props.push(row![
                botao_icone(tr!("Escolher imagem…"), "photo", Some(ev(EventoDesigner::EscolherImagem)), false),
                botao_icone(tr!("Remover"), "trash", (!c.imagem.is_empty()).then(|| ev(EventoDesigner::RemoverImagem)), false),
            ].spacing(4));
        }
        if !matches!(c.tipo, TipoControle::Rotulo | TipoControle::Moldura | TipoControle::Imagem) {
            let mut macros = vec![tr!("(nenhuma)").to_owned()];
            macros.extend(app.macro_catalogo.macros.iter().map(|m| m.name.clone()));
            props = props.push(row![
                text(if c.tipo == TipoControle::Botao { tr!("Macro") } else { tr!("Ao mudar") }).size(11).width(Length::Fixed(78.0)),
                crate::compat::pick_list_vec(macros, Some(c.macro_nome.clone().unwrap_or_else(|| tr!("(nenhuma)").to_owned())),
                    |m| ev(EventoDesigner::Campo(Campo::Macro, m))).width(Length::FILL),
            ].spacing(6).align(pelicano::Align::Center));
        }
        if c.tipo == TipoControle::Botao {
            props = props.push(marca(tr!("Limpar os campos depois"), c.limpar, Marca::Limpar));
            props = props.push(marca(tr!("Fechar o formulário"), c.fechar, Marca::Fechar));
            props = props.push(marca(tr!("Padrão (Enter)"), c.padrao, Marca::Padrao));
            props = props.push(marca(tr!("Cancelar (Esc)"), c.cancelar, Marca::Cancelar));
        }
        props = props.push(campo(tr!("Balão (dica)"), c.dica.clone(), Campo::Dica));
        // Aparência: vazio segue o tema.
        props = props.push(titulo(tr!("APARÊNCIA"), escuro));
        let aberta = |k: Campo| d.cor_aberta == Some(k);
        if c.tipo != TipoControle::Imagem {
            props = props.push(linha_cor(tr!("Fundo"), &c.fundo, Campo::Fundo, aberta(Campo::Fundo)));
            props = props.push(linha_cor(tr!("Texto"), &c.cor, Campo::Cor, aberta(Campo::Cor)));
        }
        props = props.push(linha_cor(tr!("Borda"), &c.borda, Campo::Borda, aberta(Campo::Borda)));
        props = props.push(campo(tr!("Cantos (px)"), c.raio.map(numero).unwrap_or_default(), Campo::Raio));
        if !matches!(c.tipo, TipoControle::Imagem) {
            props = props.push(campo(tr!("Letra (px)"), c.letra.map(numero).unwrap_or_default(), Campo::Letra));
            let familias = vec![tr!("Padrão").to_owned(), tr!("Serifa").to_owned(), tr!("Monoespaçada").to_owned()];
            let atual = match c.familia.as_str() { "serifa" => tr!("Serifa"), "mono" => tr!("Monoespaçada"), _ => tr!("Padrão") }.to_owned();
            props = props.push(row![
                text(tr!("Família")).size(11).width(Length::Fixed(78.0)),
                crate::compat::pick_list_vec(familias, Some(atual), |t| ev(EventoDesigner::Campo(Campo::Familia, t))).size(11).width(Length::FILL),
            ].spacing(6).align(pelicano::Align::Center));
            let alinhar = |valor: &str| ev(EventoDesigner::Campo(Campo::Alinhar, valor.to_owned()));
            let atual = ap::alinhamento(c);
            props = props.push(row![
                text(tr!("Estilo")).size(11).width(Length::Fixed(78.0)),
                botao_marca("bold", tr!("Negrito"), c.negrito, ev(EventoDesigner::Marcar(Marca::Negrito, !c.negrito))),
                botao_marca("italic", tr!("Itálico"), c.italico, ev(EventoDesigner::Marcar(Marca::Italico, !c.italico))),
                Space::new(Length::Fixed(8.0), 0.0),
                botao_marca("align-left", tr!("À esquerda"), atual == pelicano::Align::Start, alinhar("esquerda")),
                botao_marca("align-center", tr!("No centro"), atual == pelicano::Align::Center, alinhar("centro")),
                botao_marca("align-right", tr!("À direita"), atual == pelicano::Align::End, alinhar("direita")),
            ].spacing(3).align(pelicano::Align::Center));
        }
        props = props.push(titulo(tr!("COMPORTAMENTO"), escuro));
        props = props.push(marca(tr!("Desativado"), c.desativado, Marca::Desativado));
        props = props.push(marca(tr!("Oculto ao abrir"), c.oculto, Marca::Oculto));
        props = props.push(button(text(tr!("Remover controle")).size(11)).on_press(ev(EventoDesigner::RemoverControle)));
    } else {
        props = props.push(text(tr!("Clique num controle para ver as propriedades.")).size(11).color(cor_secundaria(escuro)));
    }
    if !d.aviso.is_empty() { props = props.push(text(d.aviso.clone()).size(11).color(cor_secundaria(escuro))); }
    // Só durante o arrasto o movimento vira mensagem; o invólucro fica sempre (a árvore não muda).
    let mut tudo = pelicano::mouse_area(column![
        container(barra).style(estilo_cabecalho(escuro)),
        row![
            container(scrollable(caixa)).width(Length::Fixed(182.0)).height(Length::FILL).style(estilo_cabecalho(escuro)),
            desenho,
            container(scrollable(props)).width(Length::Fixed(260.0)).height(Length::FILL).style(estilo_cabecalho(escuro)),
        ].height(Length::FILL),
    ].height(Length::FILL));
    if d.arrasto.is_some() {
        tudo = tudo.on_move(|p| ev(EventoDesigner::ArrastoMover(p.x, p.y))).on_release(ev(EventoDesigner::ArrastoFim));
    }
    tudo.into()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn desenhar_guardar_e_testar() {
        let mut app = App::default();
        app.evento_designer(EventoDesigner::Novo);
        app.evento_designer(EventoDesigner::Adicionar(TipoControle::Texto));
        app.evento_designer(EventoDesigner::Adicionar(TipoControle::Botao));
        app.evento_designer(EventoDesigner::Campo(Campo::Texto, "Gravar".into()));
        app.evento_designer(EventoDesigner::Campo(Campo::Macro, "NaoExiste".into()));
        app.evento_designer(EventoDesigner::Guardar);
        assert!(app.macro_visual.designer.aviso.contains("NaoExiste"), "{}", app.macro_visual.designer.aviso);
        assert!(app.workbook.formularios.is_none());
        app.evento_designer(EventoDesigner::Campo(Campo::Macro, tr!("(nenhuma)").into()));
        app.evento_designer(EventoDesigner::Campo(Campo::FormNome, "cadastro".into()));
        let _ = designer(&app);
        app.evento_designer(EventoDesigner::Testar);
        let lista = formularios::ler(app.workbook.formularios.as_deref()).unwrap();
        assert_eq!((lista[0].nome.as_str(), lista[0].controles[0].nome.as_str(), lista[0].controles[1].y), ("cadastro", "texto1", 42.0));
        assert!(app.formulario_macro.is_some(), "Testar abre o formulário");
        // Renomear e guardar troca o mesmo formulário, não cria outro.
        app.evento_designer(EventoDesigner::Campo(Campo::FormNome, "cliente".into()));
        app.evento_designer(EventoDesigner::Guardar);
        assert_eq!(formularios::ler(app.workbook.formularios.as_deref()).unwrap().len(), 1);
        // Arrastar move em passos da grade; a alça redimensiona.
        app.evento_designer(EventoDesigner::ArrastoInicio(0, false));
        for (x, y) in [(500.0, 300.0), (511.0, 309.0)] { app.evento_designer(EventoDesigner::ArrastoMover(x, y)); }
        app.evento_designer(EventoDesigner::ArrastoFim);
        app.evento_designer(EventoDesigner::ArrastoInicio(0, true));
        for (x, y) in [(10.0, 10.0), (30.0, 14.0)] { app.evento_designer(EventoDesigner::ArrastoMover(x, y)); }
        let c = &app.macro_visual.designer.form.as_ref().unwrap().controles[0];
        assert_eq!((c.x, c.y, c.largura, c.altura), (24.0, 16.0, 220.0, 32.0));
        app.evento_designer(EventoDesigner::Excluir);
        assert!(app.workbook.formularios.is_none());
    }
}
