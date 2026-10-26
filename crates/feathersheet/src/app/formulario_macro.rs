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
    Botao(usize),
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
        let Some(aberto) = self.formulario_macro.as_mut() else { return Task::none() };
        match evento {
            Evento::Texto(nome, t) => { aberto.valores.insert(nome, Json::String(t)); }
            Evento::Caixa(nome, b) => { aberto.valores.insert(nome, Json::Bool(b)); }
            Evento::Opcao(i) => if let Some(c) = aberto.form.controles.get(i) {
                aberto.valores.insert(c.nome.clone(), Json::String(c.texto.clone()));
            },
            Evento::Fechar => self.formulario_macro = None,
            Evento::Botao(i) => {
                let Some(c) = aberto.form.controles.get(i).cloned() else { return Task::none() };
                let nome_form = aberto.form.nome.clone();
                if let Some(nome) = c.macro_nome.as_deref().filter(|n| !n.trim().is_empty()) {
                    let Some(m) = self.macro_catalogo.obter(nome) else {
                        self.status = trf!("Macro \"{nome}\" não encontrada na biblioteca.");
                        return Task::none();
                    };
                    let (codigo, nome) = (m.code.clone(), m.name.clone());
                    self.macro_valores = Some(valores_para_macro(&aberto.form, &aberto.valores));
                    let rotulo = format!("{nome_form} › {}", c.texto);
                    let Some(r) = self.rodar_macro(historico_macros::GatilhoMacro::Botao { rotulo }, &nome, &codigo, Vec::new()) else {
                        // Esperando um Sim/Não: o formulário fica como está.
                        return Task::none();
                    };
                    self.macro_saida = r.saida.join("\n");
                    if let Some(erro) = r.erro {
                        // Erro: nada foi gravado e o formulário continua preenchido.
                        self.status = trf!("Erro ao executar {nome}: {erro}. Escritas revertidas.");
                        return Task::none();
                    }
                    self.status = trf!("Macro {nome} executada pelo formulário.");
                    self.trocar_selecao(self.linha, self.coluna);
                }
                // A macro pode ter aberto outro formulário: limpar e fechar valem só para este.
                let Some(aberto) = self.formulario_macro.as_mut().filter(|a| a.form.nome == nome_form) else { return Task::none() };
                if c.limpar { aberto.valores = valores_para_editar(&aberto.form, &Json::Null); }
                if c.fechar { self.formulario_macro = None; }
            }
        }
        Task::none()
    }

    pub(super) fn view_formulario_macro(&self) -> Element<'_, Msg> {
        let Some(aberto) = &self.formulario_macro else { return Space::new(0.0, 0.0).into() };
        let ev = Msg::FormularioMacro;
        let texto = |nome: &str| aberto.valores.get(nome).and_then(Json::as_str).unwrap_or("").to_owned();
        let camadas = aberto.form.controles.iter().enumerate().map(|(i, c)| {
            let nome = c.nome.clone();
            let w: Element<'_, Msg> = match c.tipo {
                TipoControle::Rotulo => text(c.texto.clone()).size(13).into(),
                TipoControle::Texto | TipoControle::Numero | TipoControle::Senha => {
                    let dica = if c.texto.is_empty() { &c.nome } else { &c.texto };
                    let valor = aberto.valores.get(&c.nome).and_then(Json::as_str).unwrap_or("");
                    text_input(dica, valor).secure(c.tipo == TipoControle::Senha)
                        .on_input(move |t| ev(Evento::Texto(nome.clone(), t))).size(13).width(Length::FILL).into()
                }
                TipoControle::Caixa => {
                    let marcado = aberto.valores.get(&c.nome).and_then(Json::as_bool).unwrap_or(false);
                    checkbox(c.texto.clone(), marcado).on_toggle(move |b| ev(Evento::Caixa(nome.clone(), b))).size(13).into()
                }
                TipoControle::Opcao => {
                    let marcada = (texto(&c.nome) == c.texto).then_some(i);
                    radio(c.texto.clone(), i, marcada, move |i| ev(Evento::Opcao(i))).size(13).into()
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
                    .on_press(ev(Evento::Botao(i))).into(),
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
            opcoes: Vec::new(), valor: String::new(), macro_nome: None, fechar: false, limpar: false }
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

    fn texto_campo(app: &App, nome: &str) -> String {
        app.formulario_macro.as_ref().unwrap().valores[nome].as_str().unwrap().to_owned()
    }
}
