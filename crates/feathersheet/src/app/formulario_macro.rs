//! O formulário das macros (o UserForm do VBA) aberto por
//! `sheet.showComponent("nome", dados)`: janela própria com os controles nas
//! posições gravadas. O botão roda a macro dele com o que foi preenchido em
//! `valores`; "limpar" zera os campos (cadastro em série) e "fechar" fecha.
use super::*;
use feathersheet_macros::formularios::{self, Formulario, TipoControle};
use pelicano::{checkbox, radio};
use serde_json::{Map, Value as Json};

/// Margem da janela em volta da área do formulário.
const MARGEM: f32 = 16.0;

pub(super) struct FormularioAberto {
    pub form: Formulario,
    /// Campo → valor enquanto a pessoa preenche: texto, número (ainda texto), lista e
    /// grupo de opções como texto; caixa como lógico.
    pub valores: Map<String, Json>,
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
        self.formulario_macro = Some(FormularioAberto { form, valores });
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
            Evento::Opcao(i) | Evento::Confirmar(i) | Evento::Botao(i) | Evento::PegarSelecao(i) | Evento::Item(i, _) => aberto.form.controles.get(*i).is_none_or(|c| c.desativado || c.oculto),
            _ => false,
        };
        if bloqueado { return Task::none() }
        // O controle cuja macro roda depois da mudança (`Change` do VBA), se houver.
        let mudou = match evento {
            Evento::Texto(nome, t) => {
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
                if c.limpar { aberto.valores = valores_para_editar(&aberto.form, &Json::Null); }
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
        let Some(m) = self.macro_catalogo.obter(nome) else {
            self.status = trf!("Macro \"{nome}\" não encontrada na biblioteca.");
            return false;
        };
        let (codigo, nome) = (m.code.clone(), m.name.clone());
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
                    let ligado = v.as_bool().unwrap_or(false);
                    if prop == "enabled" { c.desativado = !ligado } else { c.oculto = !ligado }
                },
            }
        }
    }

    pub(super) fn view_formulario_macro(&self) -> Element<'_, Msg> {
        let Some(aberto) = &self.formulario_macro else { return Space::new(0.0, 0.0).into() };
        let ev = Msg::FormularioMacro;
        let texto = |nome: &str| aberto.valores.get(nome).and_then(Json::as_str).unwrap_or("").to_owned();
        let camadas = aberto.form.controles.iter().enumerate().filter(|(_, c)| !c.oculto).map(|(i, c)| {
            let nome = c.nome.clone();
            let ligado = !c.desativado;
            let w: Element<'_, Msg> = match c.tipo {
                TipoControle::Rotulo => text(c.texto.clone()).size(13).into(),
                TipoControle::Intervalo => {
                    let valor = aberto.valores.get(&c.nome).and_then(Json::as_str).unwrap_or("");
                    let dica = if c.texto.is_empty() { tr!("Selecione na planilha") } else { &c.texto };
                    let mut campo = text_input(dica, valor).size(13).width(Length::FILL);
                    let mut pegar = button(feather_ui::icones::icone("table", 14.0)).padding([4, 6]).style(estilo_botao_secundario);
                    if ligado {
                        campo = campo.on_input(move |t| ev(Evento::Texto(nome.clone(), t)));
                        if c.macro_nome.is_some() { campo = campo.on_submit(ev(Evento::Confirmar(i))); }
                        pegar = pegar.on_press(ev(Evento::PegarSelecao(i)));
                    }
                    pelicano::row![campo, pelicano::tooltip(pegar, tr!("Usar a seleção da planilha"), pelicano::TooltipPosition::Top)]
                        .spacing(4).align(pelicano::Align::Center).into()
                }
                TipoControle::Texto | TipoControle::Numero | TipoControle::Senha => {
                    let dica = if c.texto.is_empty() { &c.nome } else { &c.texto };
                    let valor = aberto.valores.get(&c.nome).and_then(Json::as_str).unwrap_or("");
                    let mut campo = text_input(dica, valor).secure(c.tipo == TipoControle::Senha).size(13).width(Length::FILL);
                    if ligado {
                        campo = campo.on_input(move |t| ev(Evento::Texto(nome.clone(), t)));
                        if c.macro_nome.is_some() { campo = campo.on_submit(ev(Evento::Confirmar(i))); }
                    }
                    campo.into()
                }
                TipoControle::Caixa => {
                    let marcado = aberto.valores.get(&c.nome).and_then(Json::as_bool).unwrap_or(false);
                    let caixa = checkbox(c.texto.clone(), marcado).size(13);
                    if ligado { caixa.on_toggle(move |b| ev(Evento::Caixa(nome.clone(), b))).into() } else { caixa.into() }
                }
                TipoControle::Opcao => {
                    let marcada = (texto(&c.nome) == c.texto).then_some(i);
                    radio(c.texto.clone(), i, marcada, move |i| ev(Evento::Opcao(i))).size(13).into()
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
                            let mut caixa = checkbox(o.clone(), marcada).size(13);
                            if ligado { let o = o.clone(); caixa = caixa.on_toggle(move |_| ev(Evento::Item(i, o.clone()))); }
                            return container(caixa).padding([2, 6]).width(Length::FILL).into();
                        }
                        let linha = container(text(o.clone()).size(13).color(if marcada { pelicano::Color::WHITE } else { COR_TEXTO() }))
                            .padding([3, 6]).width(Length::FILL)
                            .style(move |_| pelicano::Style {
                                background: marcada.then(|| pelicano::Background::Color(COR_AZUL_ACENTO())),
                                ..Default::default()
                            });
                        let mut area = pelicano::mouse_area(linha);
                        if ligado { area = area.on_press(ev(Evento::Item(i, o.clone()))); }
                        area.into()
                    }).collect::<Vec<Element<'_, Msg>>>();
                    container(pelicano::scrollable(pelicano::column(linhas)).height(Length::FILL))
                        .width(Length::FILL).height(Length::FILL).padding(1)
                        .style(|_| pelicano::Style {
                            background: Some(pelicano::Background::Color(COR_PAINEL())),
                            border: pelicano::Border { color: COR_BORDA(), width: 1.0, radius: [4.0; 4] },
                            ..Default::default()
                        }).into()
                }
                TipoControle::Lista if c.livre => {
                    // Aceita digitar: o campo, com a setinha da lista ao lado.
                    let valor = aberto.valores.get(&c.nome).and_then(Json::as_str).unwrap_or("");
                    let mut campo = text_input(&c.texto, valor).size(13).width(Length::FILL);
                    let nome_setinha = i;
                    let mut row = pelicano::row![].spacing(2).align(pelicano::Align::Center);
                    if ligado {
                        campo = campo.on_input(move |t| ev(Evento::Texto(nome.clone(), t)));
                        if c.macro_nome.is_some() { campo = campo.on_submit(ev(Evento::Confirmar(i))); }
                    }
                    row = row.push(campo);
                    if ligado {
                        row = row.push(crate::compat::pick_list_vec(c.opcoes.clone(), None, move |t| ev(Evento::Item(nome_setinha, t)))
                            .width(Length::Fixed(30.0)));
                    }
                    row.into()
                }
                TipoControle::Lista => {
                    let atual = Some(texto(&c.nome)).filter(|t| !t.is_empty());
                    crate::compat::pick_list_vec(c.opcoes.clone(), atual, move |t| ev(Evento::Texto(nome.clone(), t)))
                        .width(Length::FILL).into()
                }
                TipoControle::Botao => button(container(text(c.texto.clone()).size(13)).width(Length::FILL).height(Length::FILL)
                    .align_x(pelicano::Align::Center).align_y(pelicano::Align::Center))
                    .width(Length::FILL).height(Length::FILL)
                    .style(if c.macro_nome.is_some() { estilo_botao_primario } else { estilo_botao_secundario })
                    .on_press_maybe(ligado.then(|| ev(Evento::Botao(i)))).into(),
            };
            // Desativado: esmaecido, como no VBA.
            let w: Element<'_, Msg> = if ligado { w } else {
                let mut veu = COR_PAINEL();
                veu.a = 0.55;
                pelicano::stack(vec![w, container(Space::new(Length::FILL, Length::FILL)).width(Length::FILL).height(Length::FILL)
                    .style(move |_| pelicano::Style { background: Some(pelicano::Background::Color(veu)), ..Default::default() }).into()]).into()
            };
            pelicano::column![
                Space::new(0.0, Length::Fixed(c.y)),
                pelicano::row![
                    Space::new(Length::Fixed(c.x), 0.0),
                    container(w).width(Length::Fixed(c.largura)).height(Length::Fixed(c.altura)).align_y(pelicano::Align::Center),
                ],
            ].into()
        });
        let area = pelicano::stack(camadas.collect::<Vec<Element<'_, Msg>>>())
            .width(Length::Fixed(aberto.form.largura)).height(Length::Fixed(aberto.form.altura));
        container(area).padding(MARGEM).width(Length::FILL).height(Length::FILL)
            .style(|_| pelicano::Style { background: Some(pelicano::Background::Color(COR_PAINEL())), ..Default::default() })
            .into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_macros::formularios::Controle;

    fn controle(tipo: TipoControle, nome: &str, texto: &str) -> Controle {
        Controle { tipo, nome: nome.into(), texto: texto.into(), x: 10.0, y: 10.0, largura: 120.0, altura: 26.0,
            opcoes: Vec::new(), fonte: String::new(), livre: false, multipla: false, valor: String::new(), macro_nome: None, fechar: false, limpar: false, padrao: false, cancelar: false, desativado: false, oculto: false }
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

    fn texto_campo(app: &App, nome: &str) -> String {
        app.formulario_macro.as_ref().unwrap().valores[nome].as_str().unwrap().to_owned()
    }
}
