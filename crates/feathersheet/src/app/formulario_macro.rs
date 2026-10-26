//! O formulário das macros (o UserForm do VBA) aberto por
//! `sheet.showComponent("nome", dados)`: janela própria com os controles nas
//! posições gravadas. O botão roda a macro dele com o que foi preenchido em
//! `valores`; "limpar" zera os campos (cadastro em série) e "fechar" fecha.
use super::*;
use feathersheet_macros::formularios::{self, Controle, Formulario, TipoControle};
use pelicano::{checkbox, radio};
use serde_json::{Map, Value as Json};

/// Margem da janela em volta da área do formulário.
const MARGEM: f32 = 16.0;

pub(super) struct FormularioAberto {
    pub form: Formulario,
    /// Campo → valor enquanto a pessoa preenche: texto, número (ainda texto), lista e
    /// grupo de opções como texto; caixa como lógico.
    pub valores: Map<String, Json>,
    /// O texto dos campos de várias linhas (o `text_editor` edita um `Content`); `valores` segue igual.
    pub editores: std::collections::HashMap<String, pelicano::Content>,
}

impl FormularioAberto {
    fn novo(form: Formulario, valores: Map<String, Json>) -> Self {
        let mut f = Self { form, valores, editores: Default::default() };
        f.sincronizar_editores();
        f
    }
    /// Refaz o texto dos campos de várias linhas a partir de `valores` (abrir, limpar, `field.value`).
    fn sincronizar_editores(&mut self) {
        self.editores = self.form.controles.iter().filter(|c| c.tipo == TipoControle::Texto && c.multilinha)
            .map(|c| (c.nome.clone(), pelicano::Content::with_text(self.valores.get(&c.nome).and_then(Json::as_str).unwrap_or(""))))
            .collect();
    }
}

/// A aparência gravada no controle (vazio segue o tema), usada pela janela e pela prévia do designer.
pub(in crate::app) mod aparencia {
    use feathersheet_macros::formularios::{Controle, TipoControle};
    use pelicano::{Align, Background, Color, Font, Style};

    pub fn cor(hex: &str) -> Option<Color> {
        let h = hex.trim().trim_start_matches('#');
        let h: String = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_owned() };
        let n = (h.len() == 6).then(|| u32::from_str_radix(&h, 16).ok()).flatten()?;
        Some(Color::rgb8((n >> 16) as u8, (n >> 8) as u8, n as u8))
    }

    pub fn fonte(c: &Controle) -> Font {
        let familia = match c.familia.as_str() { "serifa" => Some("serif"), "mono" => Some("monospace"), _ => None };
        Font { family: familia, weight: if c.negrito { 700 } else { 400 }, italic: c.italico }
    }

    pub fn letra(c: &Controle) -> f32 { c.letra.unwrap_or(13.0) }

    pub fn alinhamento(c: &Controle) -> Align {
        match c.alinhar.as_str() {
            "centro" => Align::Center,
            "direita" => Align::End,
            "esquerda" => Align::Start,
            _ if c.tipo == TipoControle::Botao => Align::Center,
            _ => Align::Start,
        }
    }

    /// Fundo, borda, cantos e cor do texto do controle por cima do estilo do tema.
    pub fn sobre(c: &Controle, mut st: Style) -> Style {
        if let Some(f) = cor(&c.fundo) { st.background = Some(Background::Color(f)); }
        if let Some(b) = cor(&c.borda) { st.border.color = b; st.border.width = st.border.width.max(1.0); }
        if let Some(r) = c.raio { st.border.radius = [r; 4]; }
        if let Some(t) = cor(&c.cor) { st.text_color = Some(t); }
        st
    }

    /// A imagem do controle, decodificada uma vez por conteúdo.
    pub fn imagem(b64: &str) -> Option<pelicano::Image> {
        use base64::Engine;
        use std::hash::{Hash, Hasher};
        thread_local! { static CACHE: std::cell::RefCell<std::collections::HashMap<u64, Option<pelicano::Image>>> = Default::default(); }
        if b64.is_empty() { return None; }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        b64.hash(&mut h);
        let chave = h.finish();
        CACHE.with(|c| {
            let mut c = c.borrow_mut();
            // shortcut: limpa tudo ao passar de 32 imagens; trocar por LRU se um formulário tiver mais que isso.
            if c.len() > 32 { c.clear(); }
            c.entry(chave).or_insert_with(|| {
                let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).ok()?;
                crate::compat::imagem_de_bytes(&bytes)
            }).clone()
        })
    }
}

#[derive(Debug, Clone)]
pub enum Evento {
    Texto(String, String),
    Caixa(String, bool),
    /// O índice do controle de opção marcado.
    Opcao(usize),
    /// Enter num campo de texto (o índice do controle): roda a macro dele.
    Confirmar(usize),
    Botao(usize),
    /// Clique num item da listagem, ou escolha pela setinha da lista que aceita digitar
    /// (controle, item): roda a macro do controle.
    Item(usize, String),
    /// O botão do intervalo (RefEdit): o campo `i` recebe a seleção da planilha.
    PegarSelecao(usize),
    /// O campo de várias linhas mudou (o nome dele).
    Editor(String),
    /// Enter ou Esc na janela do formulário.
    Tecla { enter: bool },
    Fechar,
}

/// Os valores de partida, para editar: número vira texto; `dados` da macro por cima.
fn valores_para_editar(form: &Formulario, dados: &Json) -> Map<String, Json> {
    let mut v = form.valores_iniciais();
    for c in form.controles.iter().filter(|c| c.tipo == TipoControle::Numero) {
        if let Some(n) = v.get(&c.nome).and_then(Json::as_f64) { v.insert(c.nome.clone(), Json::String(numero_para_texto(n))); }
        else { v.insert(c.nome.clone(), Json::String(String::new())); }
    }
    if let Some(d) = dados.as_object() {
        for (k, valor) in d {
            let Some(c) = form.controles.iter().find(|c| &c.nome == k && c.tipo.tem_valor()) else { continue };
            let novo = match (c.tipo, valor) {
                (TipoControle::Caixa, Json::Bool(b)) => Json::Bool(*b),
                // Listagem de várias escolhas: lista de textos.
                (TipoControle::Listagem, Json::Array(a)) if c.multipla =>
                    Json::Array(a.iter().map(|x| Json::String(x.as_str().map_or_else(|| x.to_string(), str::to_owned))).collect()),
                (TipoControle::Listagem, outro) if c.multipla =>
                    Json::Array(outro.as_str().filter(|s| !s.is_empty()).map(|s| Json::String(s.into())).into_iter().collect()),
                (TipoControle::Caixa, outro) => Json::Bool(!outro.is_null() && outro != &Json::from(0) && outro != &Json::from("")),
                (_, Json::String(s)) => Json::String(s.clone()),
                (_, Json::Number(n)) => Json::String(n.as_f64().map(numero_para_texto).unwrap_or_default()),
                (_, Json::Null) => Json::String(String::new()),
                (_, outro) => Json::String(outro.to_string()),
            };
            v.insert(k.clone(), novo);
        }
    }
    v
}

fn numero_para_texto(n: f64) -> String {
    feathersheet_core::celulas::number::to_js_string(n).replace('.', ",")
}

/// O que a macro recebe em `valores`: número como número (vazio ou inválido = null).
fn valores_para_macro(form: &Formulario, editando: &Map<String, Json>) -> Map<String, Json> {
    let mut v = editando.clone();
    for c in form.controles.iter().filter(|c| c.tipo == TipoControle::Numero) {
        let n = v.get(&c.nome).and_then(Json::as_str).and_then(|t| t.trim().replace('.', "").replace(',', ".").parse::<f64>().ok()
            .or_else(|| t.trim().parse::<f64>().ok()));
        v.insert(c.nome.clone(), n.map_or(Json::Null, Json::from));
    }
    v
}

impl App {
    /// Abre (ou troca) o formulário pedido pela macro.
    pub(super) fn abrir_formulario_macro(&mut self, nome: &str, dados: &Json) {
        let lista = match formularios::ler(self.workbook.formularios.as_deref()) {
            Ok(l) => l,
            Err(e) => { self.status = e; return; }
        };
        let Some(form) = formularios::obter(&lista, nome).cloned() else {
            self.status = trf!("Formulário \"{nome}\" não encontrado.");
            return;
        };
        let mut form = form;
        // RowSource: a lista lê as células na abertura, pelo mesmo caminho da lista suspensa da validação.
        for c in form.controles.iter_mut().filter(|c| matches!(c.tipo, TipoControle::Lista | TipoControle::Listagem) && !c.fonte.trim().is_empty()) {
            let regra = feathersheet_core::operacoes::validation::Regra::Formula { formula: c.fonte.clone() };
            c.opcoes = self.workbook.opcoes_da_lista(self.folha, self.linha, self.coluna, &regra).unwrap_or_default();
        }
        let valores = valores_para_editar(&form, dados);
        self.formulario_macro = Some(FormularioAberto::novo(form, valores));
    }

    pub(super) fn tamanho_formulario(&self) -> Size {
        self.formulario_macro.as_ref()
            .map_or(Self::tamanho_aux(Auxiliar::Formulario), |f| {
                // A barra de título própria fica dentro da janela.
                let barra = if self.config.barra_propria { feather_ui::cromo::BARRA_ALTURA } else { 0.0 };
                Size::new(f.form.largura + 2.0 * MARGEM, f.form.altura + 2.0 * MARGEM + barra)
            })
    }

    pub(super) fn evento_formulario_macro(&mut self, evento: Evento) -> Task<Msg> {
        // A seleção da planilha, para o botão do intervalo (calculada antes de emprestar o formulário).
        let selecao = matches!(evento, Evento::PegarSelecao(_)).then(|| {
            let aba = self.workbook.nomes().get(self.folha).cloned().unwrap_or_default();
            format!("{}!{}", feathersheet_core::celulas::refs::quote_sheet(&aba), endereco_area(&self.area_selecionada()))
        });
        let Some(aberto) = self.formulario_macro.as_mut() else { return Task::none() };
        // `Enabled = False`: o rádio e a lista não têm modo desligado no pelicano; o clique para aqui.
        let desligado = |nome: &str| aberto.form.controles.iter().any(|c| c.nome == nome && (c.desativado || c.oculto));
        let bloqueado = match &evento {
            Evento::Texto(n, _) | Evento::Caixa(n, _) => desligado(n),
            Evento::Editor(n) => desligado(n),
            Evento::Opcao(i) | Evento::Confirmar(i) | Evento::Botao(i) | Evento::PegarSelecao(i) | Evento::Item(i, _) => aberto.form.controles.get(*i).is_none_or(|c| c.desativado || c.oculto),
            _ => false,
        };
        if bloqueado { return Task::none() }
        // O controle cuja macro roda depois da mudança (`Change` do VBA), se houver.
        let mudou = match evento {
            Evento::Texto(nome, t) => {
                // MaxLength: o que passa do limite não entra.
                let max = aberto.form.controles.iter().find(|c| c.nome == nome).map_or(0, |c| c.max as usize);
                let t = if max > 0 { t.chars().take(max).collect() } else { t };
                // A lista fechada roda a macro ao escolher; a que aceita digitar, só no Enter.
                let lista = aberto.form.controles.iter().position(|c| c.nome == nome && c.tipo == TipoControle::Lista && !c.livre);
                aberto.valores.insert(nome, Json::String(t));
                lista
            }
            Evento::Caixa(nome, b) => {
                aberto.valores.insert(nome.clone(), Json::Bool(b));
                aberto.form.controles.iter().position(|c| c.nome == nome && c.tipo == TipoControle::Caixa)
            }
            Evento::Opcao(i) => aberto.form.controles.get(i).map(|c| {
                aberto.valores.insert(c.nome.clone(), Json::String(c.texto.clone()));
                i
            }),
            Evento::Confirmar(i) => Some(i),
            Evento::Editor(nome) => {
                let max = aberto.form.controles.iter().find(|c| c.nome == nome).map_or(0, |c| c.max as usize);
                let Some(conteudo) = aberto.editores.get_mut(&nome) else { return Task::none() };
                let mut t = conteudo.text();
                if max > 0 && t.chars().count() > max {
                    t = t.chars().take(max).collect();
                    *conteudo = pelicano::Content::with_text(&t);
                }
                aberto.valores.insert(nome, Json::String(t));
                None
            }
            Evento::Item(i, item) => {
                let Some(c) = aberto.form.controles.get(i) else { return Task::none() };
                let novo = if c.tipo == TipoControle::Listagem && c.multipla {
                    // Várias escolhas: o clique marca ou desmarca, na ordem dos itens.
                    let atuais: Vec<String> = aberto.valores.get(&c.nome).and_then(Json::as_array)
                        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default();
                    let marcado = |o: &String| if o == &item { !atuais.contains(o) } else { atuais.contains(o) };
                    Json::Array(c.opcoes.iter().filter(|o| marcado(o)).map(|o| Json::String(o.clone())).collect())
                } else { Json::String(item) };
                aberto.valores.insert(c.nome.clone(), novo);
                Some(i)
            }
            Evento::PegarSelecao(i) => {
                let Some(c) = aberto.form.controles.get(i) else { return Task::none() };
                aberto.valores.insert(c.nome.clone(), Json::String(selecao.unwrap_or_default()));
                Some(i)
            }
            Evento::Tecla { enter } => {
                // Enter aciona o botão padrão; Esc, o de cancelar (sem ele, fecha).
                let alvo = aberto.form.controles.iter().position(|c| c.tipo == TipoControle::Botao && !c.desativado && !c.oculto
                    && if enter { c.padrao } else { c.cancelar });
                match alvo {
                    Some(i) => return self.evento_formulario_macro(Evento::Botao(i)),
                    None if !enter => self.formulario_macro = None,
                    None => {}
                }
                None
            }
            Evento::Fechar => { self.formulario_macro = None; None }
            Evento::Botao(i) => {
                let Some(c) = aberto.form.controles.get(i).cloned() else { return Task::none() };
                let nome_form = aberto.form.nome.clone();
                if !self.rodar_macro_do_controle(i) { return Task::none() }
                // A macro pode ter aberto outro formulário: limpar e fechar valem só para este.
                let Some(aberto) = self.formulario_macro.as_mut().filter(|a| a.form.nome == nome_form) else { return Task::none() };
                if c.limpar {
                    aberto.valores = valores_para_editar(&aberto.form, &Json::Null);
                    aberto.sincronizar_editores();
                }
                if c.fechar { self.formulario_macro = None; }
                None
            }
        };
        if let Some(i) = mudou { self.rodar_macro_do_controle(i); }
        Task::none()
    }

    /// Roda a macro do controle `i` com o preenchido em `valores`. Falso se ela deu
    /// erro ou ficou esperando um Sim/Não (o formulário fica como está).
    fn rodar_macro_do_controle(&mut self, i: usize) -> bool {
        let Some(aberto) = self.formulario_macro.as_ref() else { return false };
        let Some(c) = aberto.form.controles.get(i) else { return false };
        let Some(nome) = c.macro_nome.as_deref().filter(|n| !n.trim().is_empty()) else { return true };
        let Some((nome, codigo)) = feathersheet_macros::procedimentos::codigo_da_macro(&self.macro_catalogo, nome) else {
            self.status = trf!("Macro \"{nome}\" não encontrada na biblioteca.");
            return false;
        };
        let rotulo = format!("{} › {}", aberto.form.nome, if c.texto.is_empty() { &c.nome } else { &c.texto });
        self.macro_valores = Some(valores_para_macro(&aberto.form, &aberto.valores));
        let Some(r) = self.rodar_macro(historico_macros::GatilhoMacro::Botao { rotulo }, &nome, &codigo, Vec::new()) else { return false };
        self.macro_saida = r.saida.join("\n");
        if let Some(erro) = r.erro {
            // Erro: nada foi gravado e o formulário continua preenchido.
            self.status = trf!("Erro ao executar {nome}: {erro}. Escritas revertidas.");
            return false;
        }
        self.status = trf!("Macro {nome} executada pelo formulário.");
        self.trocar_selecao(self.linha, self.coluna);
        true
    }

    /// O que a macro mudou no formulário aberto (`sheet.field(nome).value/enabled/visible`).
    pub(super) fn aplicar_campos_formulario(&mut self, campos: Vec<(String, String, Json)>) {
        let Some(aberto) = self.formulario_macro.as_mut() else { return };
        for (nome, prop, v) in campos {
            match prop.as_str() {
                "value" => {
                    let um = Json::Object(std::iter::once((nome.clone(), v)).collect());
                    if let Some(novo) = valores_para_editar(&aberto.form, &um).remove(&nome) {
                        if aberto.form.controles.iter().any(|c| c.nome == nome && c.tipo.tem_valor()) { aberto.valores.insert(nome, novo); }
                    }
                }
                _ => for c in aberto.form.controles.iter_mut().filter(|c| c.nome == nome) {
                    let texto = v.as_str().unwrap_or("").to_owned();
                    match prop.as_str() {
                        "enabled" => c.desativado = !v.as_bool().unwrap_or(false),
                        "visible" => c.oculto = !v.as_bool().unwrap_or(false),
                        // Cor inválida não muda nada (a macro já terminou; o formulário fica como estava).
                        "backColor" if formularios::cor_valida(&texto) => c.fundo = texto,
                        "foreColor" if formularios::cor_valida(&texto) => c.cor = texto,
                        "caption" => c.texto = texto,
                        _ => {}
                    }
                },
            }
        }
        aberto.sincronizar_editores();
    }

    pub(super) fn view_formulario_macro(&self) -> Element<'_, Msg> {
        let Some(aberto) = &self.formulario_macro else { return Space::new(0.0, 0.0).into() };
        let fundo_form = aparencia::cor(&aberto.form.fundo).unwrap_or(COR_PAINEL());
        // Molduras atrás: os controles de dentro ficam por cima, como no VBA.
        let mut ordem: Vec<usize> = (0..aberto.form.controles.len()).filter(|&i| !aberto.form.controles[i].oculto).collect();
        ordem.sort_by_key(|&i| aberto.form.controles[i].tipo != TipoControle::Moldura);
        let camadas = ordem.into_iter().map(|i| {
            let c = &aberto.form.controles[i];
            let w = self.controle_formulario(aberto, i, c, fundo_form);
            // ControlTipText: o balão ao parar o mouse.
            let w: Element<'_, Msg> = if c.dica.is_empty() { w } else {
                pelicano::tooltip(w, c.dica.clone(), pelicano::TooltipPosition::Top).into()
            };
            // Desativado: esmaecido, como no VBA.
            let w: Element<'_, Msg> = if !c.desativado { w } else {
                let mut veu = fundo_form;
                veu.a = 0.55;
                pelicano::stack(vec![w, container(Space::new(Length::FILL, Length::FILL)).width(Length::FILL).height(Length::FILL)
                    .style(move |_| pelicano::Style { background: Some(pelicano::Background::Color(veu)), ..Default::default() }).into()]).into()
            };
            let alto = matches!(c.tipo, TipoControle::Moldura | TipoControle::Imagem | TipoControle::Listagem) || c.multilinha;
            pelicano::column![
                Space::new(0.0, Length::Fixed(c.y)),
                pelicano::row![
                    Space::new(Length::Fixed(c.x), 0.0),
                    container(w).width(Length::Fixed(c.largura)).height(Length::Fixed(c.altura))
                        .align_y(if alto { pelicano::Align::Start } else { pelicano::Align::Center }),
                ],
            ].into()
        });
        let area = pelicano::stack(camadas.collect::<Vec<Element<'_, Msg>>>())
            .width(Length::Fixed(aberto.form.largura)).height(Length::Fixed(aberto.form.altura));
        container(area).padding(MARGEM).width(Length::FILL).height(Length::FILL)
            .style(move |_| pelicano::Style { background: Some(pelicano::Background::Color(fundo_form)), ..Default::default() })
            .into()
    }

    /// Um controle do formulário aberto, com a aparência gravada nele.
    fn controle_formulario<'a>(&'a self, aberto: &'a FormularioAberto, i: usize, c: &'a Controle, fundo_form: pelicano::Color) -> Element<'a, Msg> {
        let ev = Msg::FormularioMacro;
        let nome = c.nome.clone();
        let ligado = !c.desativado;
        let (tam, fonte) = (aparencia::letra(c), aparencia::fonte(c));
        let cor_texto = aparencia::cor(&c.cor);
        let valor = aberto.valores.get(&c.nome).and_then(Json::as_str).unwrap_or("");
        let campo_estilo = move |t: &pelicano::Theme, s: pelicano::Status| aparencia::sobre(c, pelicano::input_default(t, s));
        // Rótulo, caixa e opção: o fundo e a borda vão numa caixa em volta.
        let caixa = move |w: Element<'a, Msg>| -> Element<'a, Msg> {
            container(w).width(Length::FILL).height(Length::FILL).align_y(pelicano::Align::Center)
                .align_x(aparencia::alinhamento(c)).padding([0, if c.fundo.is_empty() && c.borda.is_empty() { 0 } else { 6 }])
                .style(move |_| aparencia::sobre(c, pelicano::Style::default())).into()
        };
        let rotulo = |t: &str| { let mut x = text(t.to_owned()).size(tam).font(fonte); if let Some(k) = cor_texto { x = x.color(k); } x };
        match c.tipo {
            TipoControle::Rotulo => caixa(rotulo(&c.texto).into()),
            TipoControle::Moldura => {
                // O título corta a borda, como no Frame do VBA.
                let borda = aparencia::cor(&c.borda).unwrap_or(COR_BORDA());
                let raio = c.raio.unwrap_or(4.0);
                let fundo = aparencia::cor(&c.fundo);
                let caixa = container(Space::new(Length::FILL, Length::FILL)).width(Length::FILL).height(Length::FILL)
                    .style(move |_| pelicano::Style {
                        background: fundo.map(pelicano::Background::Color),
                        border: pelicano::Border { color: borda, width: 1.0, radius: [raio; 4] },
                        ..Default::default()
                    });
                let mut camadas = vec![pelicano::column![Space::new(0.0, Length::Fixed(tam * 0.7)), caixa].into()];
                if !c.texto.is_empty() {
                    let atras = fundo.unwrap_or(fundo_form);
                    camadas.push(pelicano::row![Space::new(Length::Fixed(10.0), 0.0),
                        container(rotulo(&c.texto)).padding([0, 4])
                            .style(move |_| pelicano::Style { background: Some(pelicano::Background::Color(atras)), ..Default::default() })].into());
                }
                pelicano::stack(camadas).width(Length::FILL).height(Length::FILL).into()
            }
            TipoControle::Imagem => match aparencia::imagem(&c.imagem) {
                Some(img) => {
                    let mut v = pelicano::image(img).width(Length::FILL).height(Length::FILL);
                    if let Some(r) = c.raio { v = v.radius(r); }
                    container(v).width(Length::FILL).height(Length::FILL)
                        .style(move |_| aparencia::sobre(c, pelicano::Style::default())).into()
                }
                None => caixa(text(tr!("(sem imagem)")).size(11).color(COR_TEXTO_MUTED()).into()),
            },
            TipoControle::Intervalo => {
                let dica = if c.texto.is_empty() { tr!("Selecione na planilha") } else { &c.texto };
                let mut campo = text_input(dica, valor).size(tam).font(fonte).style(campo_estilo).width(Length::FILL);
                let mut pegar = button(feather_ui::icones::icone("table", 14.0)).padding([4, 6]).style(estilo_botao_secundario);
                if ligado {
                    campo = campo.on_input(move |t| ev(Evento::Texto(nome.clone(), t)));
                    if c.macro_nome.is_some() { campo = campo.on_submit(ev(Evento::Confirmar(i))); }
                    pegar = pegar.on_press(ev(Evento::PegarSelecao(i)));
                }
                pelicano::row![campo, pelicano::tooltip(pegar, tr!("Usar a seleção da planilha"), pelicano::TooltipPosition::Top)]
                    .spacing(4).align(pelicano::Align::Center).into()
            }
            TipoControle::Texto if c.multilinha => match aberto.editores.get(&c.nome) {
                Some(conteudo) => {
                    let mut ed = pelicano::text_editor(conteudo).size(tam).font(fonte).padding([6, 8]).style(campo_estilo).idle_focus(false)
                        .placeholder(&c.texto);
                    if ligado { ed = ed.on_change(ev(Evento::Editor(nome))); }
                    container(ed).width(Length::FILL).height(Length::FILL).into()
                }
                None => Space::new(0.0, 0.0).into(),
            },
            TipoControle::Texto | TipoControle::Numero | TipoControle::Senha => {
                let dica = if c.texto.is_empty() { &c.nome } else { &c.texto };
                let mut campo = text_input(dica, valor).secure(c.tipo == TipoControle::Senha).size(tam).font(fonte)
                    .style(campo_estilo).width(Length::FILL);
                if ligado {
                    campo = campo.on_input(move |t| ev(Evento::Texto(nome.clone(), t)));
                    if c.macro_nome.is_some() { campo = campo.on_submit(ev(Evento::Confirmar(i))); }
                }
                campo.into()
            }
            TipoControle::Caixa => {
                let marcado = aberto.valores.get(&c.nome).and_then(Json::as_bool).unwrap_or(false);
                let mut x = checkbox(c.texto.clone(), marcado).size(tam);
                if let Some(k) = cor_texto { x = x.color(k); }
                if ligado { x = x.on_toggle(move |b| ev(Evento::Caixa(nome.clone(), b))); }
                caixa(x.into())
            }
            TipoControle::Opcao => {
                let marcada = (valor == c.texto).then_some(i);
                let mut x = radio(c.texto.clone(), i, marcada, move |i| ev(Evento::Opcao(i))).size(tam);
                if let Some(k) = cor_texto { x = x.color(k); }
                caixa(x.into())
            }
            TipoControle::Listagem => {
                let marcadas: Vec<&str> = match aberto.valores.get(&c.nome) {
                    Some(Json::Array(a)) => a.iter().filter_map(Json::as_str).collect(),
                    Some(Json::String(s)) => vec![s.as_str()],
                    _ => Vec::new(),
                };
                let linhas = c.opcoes.iter().map(|o| {
                    let marcada = marcadas.contains(&o.as_str());
                    if c.multipla {
                        // Várias escolhas: caixas de seleção, como o ListStyle de opção do VBA.
                        let mut x = checkbox(o.clone(), marcada).size(tam);
                        if let Some(k) = cor_texto { x = x.color(k); }
                        if ligado { let o = o.clone(); x = x.on_toggle(move |_| ev(Evento::Item(i, o.clone()))); }
                        return container(x).padding([2, 6]).width(Length::FILL).into();
                    }
                    let destaque = cor_texto.unwrap_or(COR_AZUL_ACENTO());
                    let linha = container(text(o.clone()).size(tam).font(fonte)
                        .color(if marcada { pelicano::Color::WHITE } else { cor_texto.unwrap_or(COR_TEXTO()) }))
                        .padding([3, 6]).width(Length::FILL)
                        .style(move |_| pelicano::Style {
                            background: marcada.then(|| pelicano::Background::Color(destaque)),
                            ..Default::default()
                        });
                    let mut area = pelicano::mouse_area(linha);
                    if ligado { area = area.on_press(ev(Evento::Item(i, o.clone()))); }
                    area.into()
                }).collect::<Vec<Element<'_, Msg>>>();
                container(pelicano::scrollable(pelicano::column(linhas)).height(Length::FILL))
                    .width(Length::FILL).height(Length::FILL).padding(1)
                    .style(move |_| aparencia::sobre(c, pelicano::Style {
                        background: Some(pelicano::Background::Color(COR_PAINEL())),
                        border: pelicano::Border { color: COR_BORDA(), width: 1.0, radius: [4.0; 4] },
                        ..Default::default()
                    })).into()
            }
            TipoControle::Lista if c.livre => {
                // Aceita digitar: o campo, com a setinha da lista ao lado.
                let mut campo = text_input(&c.texto, valor).size(tam).font(fonte).style(campo_estilo).width(Length::FILL);
                let mut row = pelicano::row![].spacing(2).align(pelicano::Align::Center);
                if ligado {
                    campo = campo.on_input(move |t| ev(Evento::Texto(nome.clone(), t)));
                    if c.macro_nome.is_some() { campo = campo.on_submit(ev(Evento::Confirmar(i))); }
                }
                row = row.push(campo);
                if ligado {
                    row = row.push(crate::compat::pick_list_vec(c.opcoes.clone(), None, move |t| ev(Evento::Item(i, t)))
                        .style(campo_estilo).width(Length::Fixed(30.0)));
                }
                row.into()
            }
            TipoControle::Lista => {
                let atual = Some(valor.to_owned()).filter(|t| !t.is_empty());
                crate::compat::pick_list_vec(c.opcoes.clone(), atual, move |t| ev(Evento::Texto(nome.clone(), t)))
                    .size(tam).style(campo_estilo).width(Length::FILL).into()
            }
            TipoControle::Botao => {
                let base = if c.macro_nome.is_some() { estilo_botao_primario } else { estilo_botao_secundario };
                let mut t = text(c.texto.clone()).size(tam).font(fonte);
                if let Some(k) = cor_texto { t = t.color(k); }
                button(container(t).width(Length::FILL).height(Length::FILL)
                    .align_x(aparencia::alinhamento(c)).align_y(pelicano::Align::Center))
                    .width(Length::FILL).height(Length::FILL)
                    .style(move |th, st| aparencia::sobre(c, base(th, st)))
                    .on_press_maybe(ligado.then(|| ev(Evento::Botao(i)))).into()
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_macros::formularios::Controle;

    fn controle(tipo: TipoControle, nome: &str, texto: &str) -> Controle {
        Controle { tipo, nome: nome.into(), texto: texto.into(), x: 10.0, y: 10.0, largura: 120.0, altura: 26.0, ..Default::default() }
    }

    /// Cadastro em série: a macro do botão recebe `valores`, grava na planilha,
    /// o formulário limpa e continua aberto.
    #[test]
    fn show_component_abre_e_o_botao_roda_a_macro_com_valores() {
        let mut app = App::default();
        let mut form = Formulario::novo("cadastro");
        let mut ok = controle(TipoControle::Botao, "", "Gravar");
        (ok.macro_nome, ok.limpar) = (Some("Gravar".into()), true);
        form.controles = vec![controle(TipoControle::Texto, "nome", "Nome"), controle(TipoControle::Numero, "idade", ""),
            controle(TipoControle::Caixa, "ativo", "Ativo"), ok];
        app.workbook.formularios = formularios::gravar(&[form]).unwrap();
        for (nome, codigo) in [("Abrir", "sheet.showComponent('cadastro', { nome: 'Ana' });"),
            ("Gravar", "const l = sheet.getLastRow('A') + 1; sheet.set('A' + l, valores.nome); sheet.set('B' + l, valores.idade); sheet.set('C' + l, valores.ativo ? 'sim' : 'não');")] {
            app.macro_catalogo.macros.push(feather_macros::Macro { name: nome.into(), code: codigo.into(), auto: false });
        }
        let codigo = app.macro_catalogo.obter("Abrir").unwrap().code.clone();
        let r = app.rodar_macro(historico_macros::GatilhoMacro::Manual { origem: "teste".into() }, "Abrir", &codigo, Vec::new()).unwrap();
        assert!(r.erro.is_none(), "{:?}", r.erro);
        assert_eq!(texto_campo(&app, "nome"), "Ana", "os dados do showComponent preenchem o campo");
        let _ = app.evento_formulario_macro(Evento::Texto("idade".into(), "31,5".into()));
        let _ = app.evento_formulario_macro(Evento::Caixa("ativo".into(), true));
        let _ = app.evento_formulario_macro(Evento::Botao(3));
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Text("Ana".into()), "{}", app.status);
        assert_eq!(app.workbook.valor(0, 0, 1), feathersheet_core::Value::Number(31.5));
        assert_eq!(app.workbook.valor(0, 0, 2), feathersheet_core::Value::Text("sim".into()));
        assert_eq!(texto_campo(&app, "nome"), "", "limpar zera os campos");
        assert!(app.formulario_macro.is_some(), "sem fechar, continua aberto");
        let _ = app.view_formulario_macro();
    }

    /// Change da caixa liga/desliga outro campo pela macro; Enter aciona o botão padrão
    /// e Esc o de cancelar, que fecha com hideComponent.
    #[test]
    fn field_enter_esc_e_campo_desativado() {
        let mut app = App::default();
        let mut form = Formulario::novo("f");
        let mut ativo = controle(TipoControle::Caixa, "ativo", "Ativo");
        ativo.macro_nome = Some("Trocar".into());
        let mut ok = controle(TipoControle::Botao, "", "OK");
        (ok.macro_nome, ok.padrao) = (Some("Ok".into()), true);
        let mut sair = controle(TipoControle::Botao, "", "Sair");
        (sair.macro_nome, sair.cancelar) = (Some("Sair".into()), true);
        form.controles = vec![controle(TipoControle::Texto, "nome", ""), ativo, ok, sair];
        app.workbook.formularios = formularios::gravar(&[form]).unwrap();
        for (nome, codigo) in [("Trocar", "sheet.field('nome').enabled = valores.ativo; sheet.field('nome').value = valores.ativo ? 'liberado' : '';"),
            ("Ok", "sheet.set('A1', valores.nome);"), ("Sair", "sheet.hideComponent();")] {
            app.macro_catalogo.macros.push(feather_macros::Macro { name: nome.into(), code: codigo.into(), auto: false });
        }
        app.abrir_formulario_macro("f", &Json::Null);
        let _ = app.evento_formulario_macro(Evento::Caixa("ativo".into(), true));
        assert_eq!(texto_campo(&app, "nome"), "liberado", "{}", app.status);
        let _ = app.evento_formulario_macro(Evento::Caixa("ativo".into(), false));
        assert!(app.formulario_macro.as_ref().unwrap().form.controles[0].desativado);
        let _ = app.evento_formulario_macro(Evento::Texto("nome".into(), "x".into()));
        assert_eq!(texto_campo(&app, "nome"), "", "campo desativado não aceita mudança");
        let _ = app.view_formulario_macro();
        let _ = app.evento_formulario_macro(Evento::Caixa("ativo".into(), true));
        let _ = app.evento_formulario_macro(Evento::Tecla { enter: true });
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Text("liberado".into()));
        let _ = app.evento_formulario_macro(Evento::Tecla { enter: false });
        assert!(app.formulario_macro.is_none(), "Esc roda o Cancelar, que fecha");
    }

    /// RowSource: a lista lê as células (de outra aba) ao abrir; RefEdit: o botão pega a seleção.
    #[test]
    fn lista_de_celulas_e_intervalo_pela_selecao() {
        let mut app = App::default();
        app.workbook.inserir_aba("Dados", 1);
        for (l, t) in ["SP", "", "RJ"].iter().enumerate() {
            app.workbook.escrever_celula(1, l as u32, 0, t.to_string());
        }
        let mut form = Formulario::novo("f");
        let mut uf = controle(TipoControle::Lista, "uf", "");
        uf.fonte = "Dados!A1:A3".into();
        form.controles = vec![uf, controle(TipoControle::Intervalo, "area", "")];
        app.workbook.formularios = formularios::gravar(&[form]).unwrap();
        app.abrir_formulario_macro("f", &Json::Null);
        assert_eq!(app.formulario_macro.as_ref().unwrap().form.controles[0].opcoes, vec!["SP", "RJ"]);
        (app.linha, app.coluna, app.fim_selecao) = (1, 1, (4, 2));
        let _ = app.evento_formulario_macro(Evento::PegarSelecao(1));
        assert_eq!(texto_campo(&app, "area"), "Planilha1!B2:C5");
        let _ = app.view_formulario_macro();
    }

    #[test]
    fn listagem_de_varias_escolhas_e_lista_que_aceita_digitar() {
        let mut app = App::default();
        let mut form = Formulario::novo("f");
        let mut cores = controle(TipoControle::Listagem, "cores", "");
        (cores.opcoes, cores.multipla, cores.valor) = (vec!["azul".into(), "verde".into(), "roxo".into()], true, "roxo".into());
        let mut uf = controle(TipoControle::Lista, "uf", "");
        (uf.opcoes, uf.livre) = (vec!["SP".into()], true);
        form.controles = vec![cores, uf];
        app.workbook.formularios = formularios::gravar(&[form]).unwrap();
        app.abrir_formulario_macro("f", &Json::Null);
        // Marca na ordem dos itens; o segundo clique desmarca.
        let _ = app.evento_formulario_macro(Evento::Item(0, "azul".into()));
        let _ = app.evento_formulario_macro(Evento::Item(0, "roxo".into()));
        let _ = app.evento_formulario_macro(Evento::Item(0, "verde".into()));
        assert_eq!(app.formulario_macro.as_ref().unwrap().valores["cores"], serde_json::json!(["azul", "verde"]));
        let _ = app.evento_formulario_macro(Evento::Texto("uf".into(), "MG".into()));
        assert_eq!(texto_campo(&app, "uf"), "MG");
        let _ = app.evento_formulario_macro(Evento::Item(1, "SP".into()));
        assert_eq!(texto_campo(&app, "uf"), "SP");
        let _ = app.view_formulario_macro();
    }

    #[test]
    fn aparencia_limite_varias_linhas_e_cor_pela_macro() {
        let mut app = App::default();
        let mut form = Formulario::novo("f");
        form.fundo = "#fff3e0".into();
        let mut obs = controle(TipoControle::Texto, "obs", "");
        (obs.multilinha, obs.max, obs.valor) = (true, 5, "a\nb".into());
        let mut cod = controle(TipoControle::Texto, "cod", "");
        (cod.max, cod.fundo, cod.raio, cod.negrito, cod.dica) = (3, "#e3f2fd".into(), Some(10.0), true, "Código".into());
        let mut ok = controle(TipoControle::Botao, "", "OK");
        (ok.fundo, ok.cor, ok.alinhar) = ("#43a047".into(), "#ffffff".into(), "direita".into());
        form.controles = vec![controle(TipoControle::Moldura, "", "Dados"), obs, cod, ok, controle(TipoControle::Imagem, "", "")];
        app.workbook.formularios = formularios::gravar(&[form]).unwrap();
        app.abrir_formulario_macro("f", &Json::Null);
        assert_eq!(app.formulario_macro.as_ref().unwrap().editores["obs"].text(), "a\nb");
        // MaxLength corta o que passa.
        let _ = app.evento_formulario_macro(Evento::Texto("cod".into(), "12345".into()));
        assert_eq!(texto_campo(&app, "cod"), "123");
        // sheet.field: cor e legenda mudam o formulário aberto; cor inválida é ignorada.
        app.aplicar_campos_formulario(vec![("cod".into(), "backColor".into(), "#ff0000".into()), ("cod".into(), "foreColor".into(), "verde".into()),
            ("obs".into(), "value".into(), "novo".into())]);
        let aberto = app.formulario_macro.as_ref().unwrap();
        assert_eq!((aberto.form.controles[2].fundo.as_str(), aberto.form.controles[2].cor.as_str()), ("#ff0000", ""));
        assert_eq!(aberto.editores["obs"].text(), "novo");
        assert_eq!(aparencia::cor("#abc"), Some(pelicano::Color::rgb8(0xaa, 0xbb, 0xcc)));
        let _ = app.view_formulario_macro();
    }

    fn texto_campo(app: &App, nome: &str) -> String {
        app.formulario_macro.as_ref().unwrap().valores[nome].as_str().unwrap().to_owned()
    }
}
