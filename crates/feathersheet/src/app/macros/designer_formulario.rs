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
}

/// O designer do VBA encaixa numa grade; aqui, 4 px.
const GRADE: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo { FormNome, FormTitulo, FormLargura, FormAltura, Nome, Texto, X, Y, Largura, Altura, Opcoes, Valor, Macro }

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
    Limpar(bool),
    Fechar(bool),
    RemoverControle,
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
    }
}

fn icone_tipo(t: TipoControle) -> &'static str {
    match t {
        TipoControle::Rotulo => "text-size", TipoControle::Texto => "input", TipoControle::Numero => "sum",
        TipoControle::Senha => "lock", TipoControle::Caixa => "checkbox", TipoControle::Opcao => "circle-dot",
        TipoControle::Lista => "list-select", TipoControle::Botao => "square",
    }
}

fn numero(v: f32) -> String { if v.fract() == 0.0 { format!("{v:.0}") } else { v.to_string() } }

fn ler_numero(t: &str) -> f32 { t.trim().replace(',', ".").parse::<f32>().unwrap_or(0.0).clamp(0.0, 2000.0) }

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
                let (largura, altura) = match tipo { TipoControle::Botao => (110.0, 30.0), TipoControle::Rotulo => (160.0, 20.0), _ => (200.0, 26.0) };
                let texto = match tipo { TipoControle::Rotulo => tr!("Rótulo"), TipoControle::Botao => tr!("OK"),
                    TipoControle::Caixa | TipoControle::Opcao => rotulo_tipo(tipo), _ => "" };
                let nome = if tipo.tem_valor() { nome_livre(f, tipo) } else { String::new() };
                f.controles.push(Controle { tipo, nome, texto: texto.into(), x: 12.0, y: y.min((f.altura - altura).max(0.0)),
                    largura, altura, opcoes: Vec::new(), valor: String::new(), macro_nome: None, fechar: false, limpar: false });
                d.sel = Some(f.controles.len() - 1);
            },
            EventoDesigner::Selecionar(i) => d.sel = i,
            EventoDesigner::ArrastoInicio(i, alca) => { d.sel = Some(i); d.arrasto = Some((i, alca, None)); }
            EventoDesigner::ArrastoFim => d.arrasto = None,
            EventoDesigner::ArrastoMover(px, py) => if let (Some((i, alca, inicio)), Some(f)) = (d.arrasto.as_mut(), d.form.as_mut()) {
                let Some(c) = f.controles.get_mut(*i) else { return };
                let ((x0, y0), [cx, cy, cl, ca]) = *inicio.get_or_insert(((px, py), [c.x, c.y, c.largura, c.altura]));
                let encaixar = |v: f32| (v / GRADE).round() * GRADE;
                let (dx, dy) = (px - x0, py - y0);
                if *alca {
                    c.largura = encaixar(cl + dx).clamp(8.0, (f.largura - c.x).max(8.0));
                    c.altura = encaixar(ca + dy).clamp(8.0, (f.altura - c.y).max(8.0));
                } else {
                    c.x = encaixar(cx + dx).clamp(0.0, (f.largura - c.largura).max(0.0));
                    c.y = encaixar(cy + dy).clamp(0.0, (f.altura - c.altura).max(0.0));
                }
            },
            EventoDesigner::Campo(campo, t) => if let Some(f) = d.form.as_mut() {
                match campo {
                    Campo::FormNome => f.nome = t,
                    Campo::FormTitulo => f.titulo = t,
                    Campo::FormLargura => f.largura = ler_numero(&t),
                    Campo::FormAltura => f.altura = ler_numero(&t),
                    _ => if let Some(c) = d.sel.and_then(|i| f.controles.get_mut(i)) {
                        match campo {
                            Campo::Nome => c.nome = t,
                            Campo::Texto => c.texto = t,
                            Campo::X => c.x = ler_numero(&t),
                            Campo::Y => c.y = ler_numero(&t),
                            Campo::Largura => c.largura = ler_numero(&t),
                            Campo::Altura => c.altura = ler_numero(&t),
                            Campo::Opcoes => c.opcoes = t.split(';').map(|o| o.trim().to_owned()).filter(|o| !o.is_empty()).collect(),
                            Campo::Valor => c.valor = t,
                            Campo::Macro => c.macro_nome = Some(t).filter(|m| !m.is_empty() && m != tr!("(nenhuma)")),
                            _ => {}
                        }
                    },
                }
            },
            EventoDesigner::Limpar(b) => if let Some(c) = d.sel.and_then(|i| d.form.as_mut()?.controles.get_mut(i)) { c.limpar = b },
            EventoDesigner::Fechar(b) => if let Some(c) = d.sel.and_then(|i| d.form.as_mut()?.controles.get_mut(i)) { c.fechar = b },
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

fn campo<'a>(rotulo: &str, valor: String, c: Campo) -> Element<'a, Msg> {
    row![
        text(rotulo.to_owned()).size(11).width(Length::Fixed(78.0)),
        text_input("", valor).on_input(move |t| ev(EventoDesigner::Campo(c, t))).size(11).padding([4, 6]),
    ].spacing(6).align(pelicano::Align::Center).into()
}

/// A prévia de um controle no desenho (sem interação: o clique escolhe).
fn previa<'a>(c: &Controle, escuro: bool, escolhido: bool) -> Element<'a, Msg> {
    let corpo = match c.tipo {
        TipoControle::Rotulo => c.texto.clone(),
        TipoControle::Texto | TipoControle::Numero | TipoControle::Senha => if c.texto.is_empty() { c.nome.clone() } else { c.texto.clone() },
        TipoControle::Caixa => format!("☐ {}", c.texto),
        TipoControle::Opcao => format!("◯ {}", c.texto),
        TipoControle::Lista => format!("{} ▾", c.opcoes.first().cloned().unwrap_or_else(|| c.nome.clone())),
        TipoControle::Botao => c.texto.clone(),
    };
    let caixa = matches!(c.tipo, TipoControle::Texto | TipoControle::Numero | TipoControle::Senha | TipoControle::Lista | TipoControle::Botao);
    let botao = c.tipo == TipoControle::Botao;
    container(text(corpo).size(12).color(if caixa && !botao { cor_secundaria(escuro) } else { cor_texto(escuro) }))
        .padding([0, 6]).width(Length::FILL).height(Length::FILL).align_y(pelicano::Align::Center)
        .align_x(if botao { pelicano::Align::Center } else { pelicano::Align::Start })
        .style(move |_| pelicano::Style {
            background: caixa.then(|| pelicano::Background::Color(if botao { cor_chrome(escuro) } else { cor_fundo(escuro) })),
            border: if escolhido { pelicano::Border { color: cor_acento(), width: 2.0, radius: [3.0; 4] } }
                else { pelicano::Border { color: cor_borda(escuro), width: if caixa { 1.0 } else { 0.0 }, radius: [3.0; 4] } },
            ..Default::default()
        })
        .into()
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
    // O formulário no tamanho real; cada controle é escolhido pelo clique.
    let camadas: Vec<Element<'a, Msg>> = std::iter::once(
        pelicano::mouse_area(Space::new(Length::FILL, Length::FILL)).on_press(ev(EventoDesigner::Selecionar(None))).into()
    ).chain(f.controles.iter().enumerate().map(|(i, c)| {
        let escolhido = d.sel == Some(i);
        let alvo = pelicano::mouse_area(container(previa(c, escuro, escolhido)).width(Length::Fixed(c.largura)).height(Length::Fixed(c.altura)))
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
        .padding(16).style(estilo_area(escuro));
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
    ].spacing(5).padding(8);
    if let Some(c) = d.sel.and_then(|i| f.controles.get(i)) {
        props = props.push(titulo(&rotulo_tipo(c.tipo).to_uppercase(), escuro));
        if c.tipo.tem_valor() { props = props.push(campo(tr!("Nome"), c.nome.clone(), Campo::Nome)); }
        let rotulo_texto = match c.tipo { TipoControle::Texto | TipoControle::Numero | TipoControle::Senha => tr!("Dica"), _ => tr!("Texto") };
        if c.tipo != TipoControle::Lista { props = props.push(campo(rotulo_texto, c.texto.clone(), Campo::Texto)); }
        for (r, v, k) in [(tr!("X"), c.x, Campo::X), (tr!("Y"), c.y, Campo::Y), (tr!("Largura"), c.largura, Campo::Largura), (tr!("Altura"), c.altura, Campo::Altura)] {
            props = props.push(campo(r, numero(v), k));
        }
        if c.tipo == TipoControle::Lista { props = props.push(campo(tr!("Itens (;)"), c.opcoes.join("; "), Campo::Opcoes)); }
        if c.tipo.tem_valor() {
            let dica = if matches!(c.tipo, TipoControle::Caixa | TipoControle::Opcao) { tr!("Marcado (1)") } else { tr!("Valor inicial") };
            props = props.push(campo(dica, c.valor.clone(), Campo::Valor));
        }
        if c.tipo == TipoControle::Botao {
            let mut macros = vec![tr!("(nenhuma)").to_owned()];
            macros.extend(app.macro_catalogo.macros.iter().map(|m| m.name.clone()));
            props = props.push(row![
                text(tr!("Macro")).size(11).width(Length::Fixed(78.0)),
                crate::compat::pick_list_vec(macros, Some(c.macro_nome.clone().unwrap_or_else(|| tr!("(nenhuma)").to_owned())),
                    |m| ev(EventoDesigner::Campo(Campo::Macro, m))).width(Length::FILL),
            ].spacing(6).align(pelicano::Align::Center));
            props = props.push(checkbox(tr!("Limpar os campos depois"), c.limpar).on_toggle(|b| ev(EventoDesigner::Limpar(b))).size(12));
            props = props.push(checkbox(tr!("Fechar o formulário"), c.fechar).on_toggle(|b| ev(EventoDesigner::Fechar(b))).size(12));
        }
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
