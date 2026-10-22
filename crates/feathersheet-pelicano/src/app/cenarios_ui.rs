//! Gerenciador de cenários (Dados → Previsão), como no Excel: lista de
//! cenários à esquerda, o cenário escolhido à direita (nome, células
//! variáveis, valores, comentário), "Mostrar" e "Resumo". Os cenários ficam
//! na aba (`Sheet::ce`, JSON) e vão para o `.fshz`; aplicar e resumir são do
//! motor (`operacoes::cenarios`).
//!
//! Não bloqueia a planilha: com o cursor em "Células variáveis" ou em
//! "Células de resultado", clicar na grade acrescenta o endereço.
use super::*;
use feathersheet_core::celulas::{refs::{parse_cell_ref, parse_range_ref}, values::{to_text, Value}};
use feathersheet_core::operacoes::cenarios::{aplicar, resumo, Cenario, MAX_CELULAS};
use pelicano::column;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct CenarioSalvo {
    pub nome: String,
    pub celulas: Vec<String>,
    pub valores: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comentario: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo { Celulas, Resultados }

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Selecionar(usize),
    Adicionar,
    Excluir,
    Nome(String),
    Celulas(String),
    Valor(usize, String),
    Comentario(String),
    Resultados(String),
    Mostrar,
    Resumo,
    Fechar,
}

#[derive(Debug, Clone)]
pub(super) struct Estado {
    pub aba: usize,
    pub lista: Vec<CenarioSalvo>,
    pub selecionado: Option<usize>,
    /// Texto do campo "Células variáveis" do cenário escolhido.
    pub celulas: String,
    pub resultados: String,
    pub campo: Campo,
    pub erro: Option<String>,
}

/// "B3;B4", "B3:B5" ou "B3, C3" viram a lista de endereços (até 32).
fn ler_celulas(texto: &str) -> Result<Vec<(u32, u32)>, String> {
    let mut saida = Vec::new();
    for parte in texto.split([';', ',', ' ']).map(str::trim).filter(|p| !p.is_empty()) {
        let parte = parte.replace('$', "").to_uppercase();
        if let Some(p) = parse_cell_ref(&parte) {
            if !saida.contains(&p) { saida.push(p); }
        } else if let Some(r) = parse_range_ref(&parte) {
            let r = r.normalized();
            for l in r.r0..=r.r1 {
                for c in r.c0..=r.c1 {
                    if saida.len() > MAX_CELULAS { break; }
                    if !saida.contains(&(l, c)) { saida.push((l, c)); }
                }
            }
        } else {
            return Err(trf!("\"{parte}\" não é uma célula nem um intervalo."));
        }
    }
    if saida.len() > MAX_CELULAS {
        return Err(trf!("Um cenário aceita até {MAX_CELULAS} células variáveis."));
    }
    Ok(saida)
}

pub(super) fn ler(sheet: &feathersheet_core::planilha::sheet::Sheet) -> Vec<CenarioSalvo> {
    sheet.ce.as_deref().and_then(|j| serde_json::from_str(j).ok()).unwrap_or_default()
}

impl App {
    pub(super) fn evento_cenarios(&mut self, evento: Evento) -> Task<Msg> {
        if let Evento::Abrir = evento {
            if !self.confirmar() { return Task::none(); }
            let lista = self.workbook.aba(self.folha).map(|a| ler(&a.sheet)).unwrap_or_default();
            let selecionado = (!lista.is_empty()).then_some(0);
            let celulas = lista.first().map(|c| c.celulas.join(";")).unwrap_or_default();
            self.cenarios = Some(Estado { aba: self.folha, lista, selecionado, celulas,
                resultados: String::new(), campo: Campo::Celulas, erro: None });
            return Task::none();
        }
        let Some(e) = self.cenarios.as_mut() else { return Task::none() };
        e.erro = None;
        let mut gravar = false;
        match evento {
            Evento::Abrir => {}
            Evento::Selecionar(i) => if i < e.lista.len() {
                e.selecionado = Some(i);
                e.celulas = e.lista[i].celulas.join(";");
                e.campo = Campo::Celulas;
            },
            Evento::Adicionar => {
                // O novo cenário começa com a seleção e os valores de agora.
                let area = self.area_selecionada();
                let celulas: Vec<(u32, u32)> = (area.r0..=area.r1)
                    .flat_map(|r| (area.c0..=area.c1).map(move |c| (r, c)))
                    .take(MAX_CELULAS).collect();
                let Some(e) = self.cenarios.as_mut() else { return Task::none() };
                let sheet = &self.workbook.aba(e.aba).expect("aba do cenário").sheet;
                let mut n = e.lista.len() + 1;
                while e.lista.iter().any(|c| c.nome == trf!("Cenário {n}")) { n += 1; }
                e.lista.push(CenarioSalvo {
                    nome: trf!("Cenário {n}"),
                    celulas: celulas.iter().map(|&(r, c)| cell_name(r, c)).collect(),
                    valores: celulas.iter().map(|&(r, c)| sheet.get_raw(r, c)).collect(),
                    comentario: String::new(),
                });
                let i = e.lista.len() - 1;
                e.selecionado = Some(i);
                e.celulas = e.lista[i].celulas.join(";");
                gravar = true;
            }
            Evento::Excluir => if let Some(i) = e.selecionado {
                e.lista.remove(i);
                e.selecionado = (!e.lista.is_empty()).then(|| i.min(e.lista.len() - 1));
                e.celulas = e.selecionado.map(|i| e.lista[i].celulas.join(";")).unwrap_or_default();
                gravar = true;
            },
            Evento::Nome(t) => if let Some(i) = e.selecionado {
                if e.lista.iter().enumerate().any(|(j, c)| j != i && c.nome == t.trim()) {
                    e.erro = Some(tr!("Já existe um cenário com esse nome.").into());
                }
                e.lista[i].nome = t;
                gravar = true;
            },
            Evento::Celulas(t) => {
                e.celulas = t;
                e.campo = Campo::Celulas;
                gravar = self.atualizar_celulas_cenario();
            }
            Evento::Valor(j, t) => if let Some(i) = e.selecionado {
                if let Some(v) = e.lista[i].valores.get_mut(j) { *v = t; gravar = true; }
            },
            Evento::Comentario(t) => if let Some(i) = e.selecionado {
                e.lista[i].comentario = t;
                gravar = true;
            },
            Evento::Resultados(t) => { e.resultados = t; e.campo = Campo::Resultados; }
            Evento::Mostrar => {
                let Some(cen) = e.selecionado.map(|i| e.lista[i].clone()) else { return Task::none() };
                let aba = e.aba;
                match converter(&cen) {
                    Ok(c) => {
                        aplicar(&mut self.workbook, aba, &c);
                        self.alterado = true;
                        self.status = trf!("Cenário \"{}\" aplicado (Ctrl+Z desfaz)", cen.nome);
                    }
                    Err(m) => if let Some(e) = &mut self.cenarios { e.erro = Some(m); },
                }
            }
            Evento::Resumo => {
                if let Err(m) = self.resumo_cenarios() {
                    if let Some(e) = &mut self.cenarios { e.erro = Some(m); }
                }
            }
            Evento::Fechar => self.cenarios = None,
        }
        if gravar { self.gravar_cenarios(); }
        Task::none()
    }

    /// Relê "Células variáveis"; valores de células que continuam ficam,
    /// os das novas vêm do conteúdo atual da célula.
    fn atualizar_celulas_cenario(&mut self) -> bool {
        let Some(e) = self.cenarios.as_mut() else { return false };
        let Some(i) = e.selecionado else { return false };
        let celulas = match ler_celulas(&e.celulas) {
            Ok(c) => c,
            Err(m) => { e.erro = Some(m); return false; }
        };
        let sheet = &self.workbook.aba(e.aba).expect("aba do cenário").sheet;
        let antigo = e.lista[i].clone();
        let nomes: Vec<String> = celulas.iter().map(|&(r, c)| cell_name(r, c)).collect();
        e.lista[i].valores = celulas.iter().zip(&nomes).map(|(&(r, c), nome)| {
            antigo.celulas.iter().position(|x| x == nome)
                .and_then(|k| antigo.valores.get(k).cloned())
                .unwrap_or_else(|| sheet.get_raw(r, c))
        }).collect();
        e.lista[i].celulas = nomes;
        true
    }

    fn gravar_cenarios(&mut self) {
        let Some(e) = &self.cenarios else { return };
        let json = (!e.lista.is_empty()).then(|| serde_json::to_string(&e.lista).unwrap_or_default());
        if let Some(aba) = self.workbook.aba_mut(e.aba) { aba.sheet.ce = json; }
        self.alterado = true;
    }

    /// Clique na grade: acrescenta o endereço ao campo ativo.
    pub(super) fn cenarios_clique(&mut self, r: u32, c: u32) {
        let Some(e) = self.cenarios.as_mut() else { return };
        if self.folha != e.aba { return; }
        let endereco = cell_name(r, c);
        let acrescentar = |t: &mut String| {
            if !t.split(';').any(|p| p.trim().eq_ignore_ascii_case(&endereco)) {
                if !t.trim().is_empty() { t.push(';'); }
                t.push_str(&endereco);
            }
        };
        match e.campo {
            Campo::Celulas if e.selecionado.is_some() => {
                acrescentar(&mut e.celulas);
                if self.atualizar_celulas_cenario() { self.gravar_cenarios(); }
            }
            Campo::Resultados => acrescentar(&mut e.resultados),
            _ => {}
        }
    }

    /// Cria a aba "Resumo do cenário" com os valores atuais e os de cada
    /// cenário, nas células variáveis e nas de resultado.
    fn resumo_cenarios(&mut self) -> Result<(), String> {
        let e = self.cenarios.clone().expect("janela aberta");
        if e.lista.is_empty() { return Err(tr!("Adicione ao menos um cenário.").into()); }
        let resultados = ler_celulas(&e.resultados)?;
        if resultados.is_empty() { return Err(tr!("Informe as células de resultado (ex.: B10).").into()); }
        let cenarios: Vec<Cenario> = e.lista.iter().map(converter).collect::<Result<_, _>>()?;
        let mut variaveis: Vec<(u32, u32)> = Vec::new();
        for c in &cenarios {
            for (p, _) in &c.valores { if !variaveis.contains(p) { variaveis.push(*p); } }
        }
        let tabela = resumo(&mut self.workbook, e.aba, &cenarios, &resultados);
        let atual = |p: (u32, u32)| to_text(&self.workbook.valor(e.aba, p.0, p.1));
        // Linhas: título, cabeçalho, células variáveis, células de resultado.
        let mut linhas: Vec<Vec<String>> = vec![vec![tr!("Resumo do cenário").into()],
            std::iter::once(String::new()).chain(std::iter::once(tr!("Valores atuais").into()))
                .chain(e.lista.iter().map(|c| c.nome.clone())).collect(),
            vec![tr!("Células variáveis:").into()]];
        for &p in &variaveis {
            let mut l = vec![cell_name(p.0, p.1), atual(p)];
            for c in &cenarios {
                l.push(c.valores.iter().find(|(q, _)| *q == p).map(|(_, v)| v.clone()).unwrap_or_default());
            }
            linhas.push(l);
        }
        linhas.push(vec![tr!("Células de resultado:").into()]);
        for (k, &p) in resultados.iter().enumerate() {
            linhas.push(std::iter::once(cell_name(p.0, p.1))
                .chain(tabela[k].iter().map(|v: &Value| to_text(v))).collect());
        }
        let existentes = self.workbook.nomes();
        let mut nome = tr!("Resumo do cenário").to_string();
        let mut n = 2;
        while existentes.contains(&nome) { nome = trf!("Resumo do cenário {n}"); n += 1; }
        let nova = self.workbook.criar_aba(nome);
        self.workbook.editar(nova, |s| {
            for (r, l) in linhas.iter().enumerate() {
                for (c, t) in l.iter().enumerate() { if !t.is_empty() { s.set_cell(r as u32, c as u32, t); } }
            }
        });
        self.alterado = true;
        self.cenarios = None;
        let _ = self.update(Msg::Aba(nova));
        self.status = trf!("Resumo de {} cenário(s) criado", e.lista.len());
        Ok(())
    }

    fn view_cenarios(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let e = self.cenarios.as_ref().expect("janela aberta");
        let ev = Msg::Cenarios;
        let destaque = |ativo: bool| move |tema: &pelicano::Theme, status| {
            let mut s = pelicano::input_default(tema, status);
            if ativo { s.border.color = COR_AZUL_ACENTO(); }
            s
        };
        let rotulo = |t: &'static str| text(t).size(12.5).width(Length::Fixed(132.0));

        // Lista à esquerda.
        let mut lista = column![text(tr!("CENÁRIOS")).size(10.5).color(COR_TEXTO_MUTED())].spacing(2);
        if e.lista.is_empty() {
            lista = lista.push(text(tr!("Nenhum cenário nesta aba.")).size(12).color(COR_TEXTO_MUTED()));
        }
        for (i, c) in e.lista.iter().enumerate() {
            let nome: String = if c.nome.chars().count() > 22 {
                format!("{}…", c.nome.chars().take(21).collect::<String>())
            } else { c.nome.clone() };
            lista = lista.push(button(text(nome).size(12.5)).width(Length::FILL).padding([6, 9])
                .style(estilo_botao_pequeno(e.selecionado == Some(i)))
                .on_press(ev(Evento::Selecionar(i))));
        }
        let esquerda = column![
            container(rolagem(lista)).height(Length::Fixed(220.0)),
            row![
                button(row![feather_ui::icones::icone("plus", 14.0), text(tr!("Adicionar")).size(12)]
                        .spacing(5).align(pelicano::Align::Center))
                    .padding([5, 9]).style(estilo_botao_secundario).on_press(ev(Evento::Adicionar)),
                button(row![feather_ui::icones::icone("trash", 14.0), text(tr!("Excluir")).size(12)]
                        .spacing(5).align(pelicano::Align::Center))
                    .padding([5, 9]).style(estilo_botao_secundario)
                    .on_press_maybe(e.selecionado.map(|_| ev(Evento::Excluir))),
            ].spacing(6),
        ].spacing(8).width(Length::Fixed(200.0));

        // Formulário do cenário escolhido.
        let direita: Element<'_, Msg> = match e.selecionado.and_then(|i| e.lista.get(i)) {
            None => container(text(tr!("Adicione um cenário: ele começa com as células selecionadas \
                e os valores de agora.")).size(12).color(COR_TEXTO_MUTED()))
                .width(Length::FILL).into(),
            Some(c) => {
                let mut valores = pelicano::column(Vec::<Element<_>>::new()).spacing(6);
                for (j, (cel, v)) in c.celulas.iter().zip(&c.valores).enumerate() {
                    valores = valores.push(row![
                        text(cel.clone()).size(12).width(Length::Fixed(60.0)),
                        text_input("", v).on_input(move |t| Msg::Cenarios(Evento::Valor(j, t)))
                            .size(12.5).padding([4, 7]).width(Length::FILL),
                    ].spacing(8).align(pelicano::Align::Center));
                }
                column![
                    row![rotulo(tr!("Nome:")), text_input("", &c.nome)
                        .on_input(|t| Msg::Cenarios(Evento::Nome(t))).size(13).padding([5, 8])]
                        .spacing(8).align(pelicano::Align::Center),
                    row![rotulo(tr!("Células variáveis:")), text_input(tr!("ex.: B3;B4 ou B3:B5"), &e.celulas)
                        .on_input(|t| Msg::Cenarios(Evento::Celulas(t))).size(13).padding([5, 8])
                        .style(destaque(e.campo == Campo::Celulas))]
                        .spacing(8).align(pelicano::Align::Center),
                    text("VALORES").size(10.5).color(COR_TEXTO_MUTED()),
                    container(rolagem(valores)).max_height(150.0),
                    row![rotulo(tr!("Comentário:")), text_input("opcional", &c.comentario)
                        .on_input(|t| Msg::Cenarios(Evento::Comentario(t))).size(13).padding([5, 8])]
                        .spacing(8).align(pelicano::Align::Center),
                ].spacing(10).width(Length::FILL).into()
            }
        };

        let mut corpo = column![
            row![esquerda, container(Space::new(1, 0.0)).height(Length::Fixed(250.0)).style(estilo_linha),
                direita].spacing(14),
            container(Space::new(0.0, 1)).width(Length::FILL).style(estilo_linha),
            row![
                rotulo(tr!("Células de resultado:")),
                text_input(tr!("ex.: B10"), &e.resultados).on_input(|t| Msg::Cenarios(Evento::Resultados(t)))
                    .size(13).padding([5, 8]).width(Length::FILL)
                    .style(destaque(e.campo == Campo::Resultados)),
                button(text(tr!("Criar resumo")).size(12.5)).padding([6, 12])
                    .style(estilo_botao_secundario).on_press(ev(Evento::Resumo)),
            ].spacing(8).align(pelicano::Align::Center),
            row![
                feather_ui::icones::icone("info-circle", 14.0),
                text(tr!("Clique na planilha para acrescentar endereços ao campo destacado."))
                    .size(11.5).color(COR_TEXTO_MUTED()),
            ].spacing(6).align(pelicano::Align::Center),
        ].spacing(14);
        if let Some(erro) = &e.erro {
            corpo = corpo.push(row![
                feather_ui::icones::icone("info-circle", 15.0), text(erro.clone()).size(12).color(COR_ERRO()),
            ].spacing(6).align(pelicano::Align::Center));
        }
        // Não modal e à direita: a pessoa clica na planilha para escolher células.
        self.dialogo(tr!("Gerenciador de cenários"), corpo)
            .topo(14.0)
            .altura_maxima((self.tamanho_principal.height - self.topo_dialogo() - 28.0).max(320.0))
            .largura(640.0)
            .nao_modal()
            .a_direita()
            .ao_fechar(ev(Evento::Fechar))
            .primario(tr!("Mostrar"), e.selecionado.map(|_| ev(Evento::Mostrar)))
    }

    pub(super) fn sobrepor_cenarios<'a>(&'a self, centro: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.cenarios.is_none() { return centro; }
        self.view_cenarios().sobre(centro)
    }
}

/// Cenário salvo → o que o motor entende (endereços já conferidos).
fn converter(c: &CenarioSalvo) -> Result<Cenario, String> {
    let celulas = ler_celulas(&c.celulas.join(";"))
        .map_err(|m| trf!("Cenário \"{}\": {m}", c.nome))?;
    Ok(Cenario { nome: c.nome.clone(),
        valores: celulas.into_iter().zip(c.valores.iter().cloned()).collect() })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn planilha() -> App {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "10".into());      // A1
        app.workbook.escrever_celula(0, 1, 0, "3".into());       // A2
        app.workbook.escrever_celula(0, 2, 0, "=A1*A2".into());  // A3
        app
    }

    #[test]
    fn adicionar_editar_mostrar_e_desfazer() {
        let mut app = planilha();
        let _ = app.update(Msg::IrPara("A1:A2".into()));
        let _ = app.update(Msg::Cenarios(Evento::Abrir));
        let _ = app.update(Msg::Cenarios(Evento::Adicionar));
        let _ = app.update(Msg::Cenarios(Evento::Nome("Alta".into())));
        let _ = app.update(Msg::Cenarios(Evento::Valor(0, "20".into())));
        let _ = app.update(Msg::Cenarios(Evento::Valor(1, "5".into())));
        let _: Element<'_, Msg> = app.view_cenarios().painel();
        let _ = app.update(Msg::Cenarios(Evento::Mostrar));
        assert_eq!(app.workbook.valor(0, 2, 0), Value::Number(100.));
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.valor(0, 2, 0), Value::Number(30.));
        let salvos = ler(&app.workbook.aba(0).unwrap().sheet);
        assert_eq!(salvos[0].nome, "Alta");
        assert_eq!(salvos[0].valores, ["20", "5"]);
    }

    #[test]
    fn celulas_por_intervalo_mantem_valores_e_resumo_cria_aba() {
        let mut app = planilha();
        let _ = app.update(Msg::Selecionar(0, 0));
        let _ = app.update(Msg::Cenarios(Evento::Abrir));
        let _ = app.update(Msg::Cenarios(Evento::Adicionar));
        let _ = app.update(Msg::Cenarios(Evento::Valor(0, "20".into())));
        let _ = app.update(Msg::Cenarios(Evento::Celulas("A1:A2".into())));
        let e = app.cenarios.as_ref().unwrap();
        assert_eq!(e.lista[0].celulas, ["A1", "A2"]);
        assert_eq!(e.lista[0].valores, ["20", "3"], "A1 continua com o valor editado");
        // Clique na grade preenche "Células de resultado".
        let _ = app.update(Msg::Cenarios(Evento::Resultados(String::new())));
        let _ = app.update(Msg::Selecionar(2, 0));
        assert_eq!(app.cenarios.as_ref().unwrap().resultados, "A3");
        let _ = app.update(Msg::Cenarios(Evento::Resumo));
        assert!(app.cenarios.is_none());
        assert_eq!(app.workbook.len(), 2);
        assert_eq!(app.folha, 1);
        // Linha do resultado: A3 | atual 30 | cenário 60.
        let linha = (0..10).find(|&r| app.workbook.aba(1).unwrap().sheet.get_raw(r, 0) == "A3").unwrap();
        assert_eq!(app.workbook.valor(1, linha, 1), Value::Number(30.));
        assert_eq!(app.workbook.valor(1, linha, 2), Value::Number(60.));
        assert_eq!(app.workbook.valor(0, 2, 0), Value::Number(30.), "a aba original volta");
    }

    #[test]
    fn cenarios_sobrevivem_a_salvar_e_abrir_o_fshz() {
        let mut app = planilha();
        let _ = app.update(Msg::IrPara("A1".into()));
        let _ = app.update(Msg::Cenarios(Evento::Abrir));
        let _ = app.update(Msg::Cenarios(Evento::Adicionar));
        let _ = app.update(Msg::Cenarios(Evento::Nome("Teste".into())));
        let dir = std::env::temp_dir().join(format!("feathersheet-cenarios-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cenarios.fshz");
        let _ = app.gravar(path.clone());
        app.concluir_gravacao_teste(path.clone(), false);
        let bytes = std::fs::read(&path).unwrap();
        let mut outro = App::default();
        outro.carregar(path.clone(), bytes).unwrap();
        assert_eq!(ler(&outro.workbook.aba(0).unwrap().sheet)[0].nome, "Teste");
        std::fs::remove_dir_all(dir).unwrap();
        for escuro in [false, true] {
            feather_ui::tema::definir_tema(escuro, 0);
            let _: Element<'_, Msg> = app.view_cenarios().painel();
        }
    }
}
