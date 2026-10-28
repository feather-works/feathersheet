//! Modal de uma etapa com parâmetros (filtrar, renomear, manter linhas,
//! dividir, ordenar, tipo, agrupar): cria a etapa pela faixa ou pelo menu da
//! coluna, e edita uma etapa já aplicada pela engrenagem, como no Power Query.
use super::*;
use pelicano::{column, row};

/// Etapas que têm modal (as outras são aplicadas direto).
#[cfg(test)]
pub(super) const COM_MODAL: [&str; 7] = ["filtrar", "renomear", "limite", "dividir", "ordenar", "tipar", "agrupar"];

const AGREGACOES: [(&str, Agregacao); 6] = [
    ("Contar linhas", Agregacao::ContarValores),
    ("Soma", Agregacao::Soma),
    ("Média", Agregacao::Media),
    ("Mínimo", Agregacao::Minimo),
    ("Máximo", Agregacao::Maximo),
    ("Contar distintos", Agregacao::Distintos),
];

#[derive(Debug, Clone)]
pub enum Evento {
    Texto(String),
    Coluna(usize),
    EmLinhas(bool),
    Decrescente(bool),
    Para(&'static str),
    /// Liga/desliga uma coluna do "Agrupar por".
    Grupo(usize),
    Agregacao(usize),
    ColunaAgregada(usize),
    NomeNovo(String),
    Aplicar,
    Fechar,
}

pub(super) struct Modal {
    pub tipo: &'static str,
    /// `Some(i)`: edita a etapa `i` em vez de acrescentar.
    pub editar: Option<usize>,
    /// Colunas que chegam a esta etapa.
    colunas: Vec<String>,
    col: usize,
    /// Filtro, novo nome, quantidade ou separador, conforme o tipo.
    texto: String,
    em_linhas: bool,
    desc: bool,
    para: &'static str,
    grupo: Vec<usize>,
    agregacao: usize,
    agregada: usize,
    nome_novo: String,
}

impl Modal {
    /// Etapa nova sobre `colunas` (o resultado atual), com `selecionadas` em destaque.
    pub fn novo(tipo: &'static str, colunas: Vec<String>, col: usize, selecionadas: &[usize]) -> Self {
        let texto = match tipo {
            "renomear" => colunas.get(col).cloned().unwrap_or_default(),
            "limite" => "100".into(),
            "dividir" => " ".into(),
            _ => String::new(),
        };
        let grupo = if selecionadas.is_empty() { vec![col] } else { selecionadas.to_vec() };
        Self {
            tipo, editar: None, colunas, col, texto, em_linhas: false, desc: false, para: "texto",
            grupo, agregacao: 0, agregada: col, nome_novo: tr!("Quantidade").into(),
        }
    }

    /// A etapa `i` aberta para edição; `None` se ela não tem modal.
    pub fn de_passo(passo: &Passo, i: usize, colunas: Vec<String>) -> Option<Self> {
        let mut m = Self::novo("filtrar", colunas, 0, &[]);
        m.editar = Some(i);
        match passo {
            Passo::Filtrar { col, criterio } => {
                m.col = *col;
                m.texto = feathersheet_core::celulas::values::to_text(criterio);
            }
            Passo::Renomear { col, nome } => { m.tipo = "renomear"; m.col = *col; m.texto = nome.clone(); }
            Passo::Limite { quantas } => { m.tipo = "limite"; m.texto = quantas.to_string(); }
            Passo::Dividir { col, separador } | Passo::DividirColunas { col, separador } => {
                m.tipo = "dividir";
                m.col = *col;
                m.texto = separador.clone();
                m.em_linhas = matches!(passo, Passo::Dividir { .. });
            }
            Passo::Ordenar { por } => {
                m.tipo = "ordenar";
                let chave = por.first()?;
                m.col = chave.col;
                m.desc = chave.desc == Some(true);
            }
            Passo::Tipar { col, para } => {
                m.tipo = "tipar";
                m.col = *col;
                m.para = if para == "numero" { "numero" } else { "texto" };
            }
            Passo::Agrupar { por, agregar } => {
                m.tipo = "agrupar";
                m.grupo = por.clone();
                let a = agregar.first()?;
                m.agregacao = AGREGACOES.iter().position(|(_, g)| *g == a.como).unwrap_or(0);
                m.agregada = a.col;
                m.nome_novo = a.nome.clone().unwrap_or_default();
            }
            _ => return None,
        }
        Some(m)
    }

    pub fn atualizar(&mut self, ev: Evento) {
        match ev {
            Evento::Texto(t) => self.texto = t,
            Evento::Coluna(c) => {
                if self.tipo == "renomear" && self.colunas.get(self.col) == Some(&self.texto) {
                    self.texto = self.colunas.get(c).cloned().unwrap_or_default();
                }
                self.col = c;
            }
            Evento::EmLinhas(v) => self.em_linhas = v,
            Evento::Decrescente(v) => self.desc = v,
            Evento::Para(p) => self.para = p,
            Evento::Grupo(c) => match self.grupo.iter().position(|g| *g == c) {
                Some(i) => { self.grupo.remove(i); }
                None => { self.grupo.push(c); self.grupo.sort_unstable(); }
            },
            Evento::Agregacao(i) => self.agregacao = i,
            Evento::ColunaAgregada(c) => self.agregada = c,
            Evento::NomeNovo(n) => self.nome_novo = n,
            Evento::Aplicar | Evento::Fechar => {}
        }
    }

    pub fn passo(&self) -> Result<Passo, String> {
        let col = self.col;
        Ok(match self.tipo {
            "filtrar" => Passo::Filtrar { col, criterio: Value::Text(self.texto.clone()) },
            "renomear" => {
                if self.texto.trim().is_empty() {
                    return Err(tr!("Informe o novo nome da coluna").into());
                }
                Passo::Renomear { col, nome: self.texto.trim().into() }
            }
            "limite" => {
                let n = self.texto.trim().replace(',', ".").parse::<f64>()
                    .map_err(|_| tr!("Quantidade de linhas inválida").to_string())?;
                if !n.is_finite() || n < 0.0 {
                    return Err(tr!("Informe um limite não negativo e finito").into());
                }
                Passo::Limite { quantas: n }
            }
            "dividir" => {
                if self.texto.is_empty() {
                    return Err(tr!("Informe um separador antes de dividir").into());
                }
                let separador = self.texto.clone();
                if self.em_linhas { Passo::Dividir { col, separador } } else { Passo::DividirColunas { col, separador } }
            }
            "ordenar" => Passo::Ordenar { por: vec![ChaveOrdenacao { col, desc: Some(self.desc) }] },
            "tipar" => Passo::Tipar { col, para: self.para.into() },
            "agrupar" => {
                if self.grupo.is_empty() {
                    return Err(tr!("Escolha ao menos uma coluna em Agrupar por").into());
                }
                Passo::Agrupar {
                    por: self.grupo.clone(),
                    agregar: vec![Agregado {
                        col: self.agregada,
                        como: AGREGACOES[self.agregacao].1,
                        nome: Some(self.nome_novo.trim().to_owned()).filter(|n| !n.is_empty()),
                    }],
                }
            }
            _ => return Err(tr!("Etapa sem parâmetros").into()),
        })
    }

    fn titulo(&self) -> &'static str {
        match self.tipo {
            "filtrar" => tr!("Filtrar linhas"),
            "renomear" => tr!("Renomear coluna"),
            "limite" => tr!("Manter primeiras linhas"),
            "dividir" => tr!("Dividir coluna por delimitador"),
            "ordenar" => tr!("Classificar"),
            "tipar" => tr!("Alterar tipo"),
            _ => tr!("Agrupar por"),
        }
    }
}

fn ev(e: Evento) -> Msg {
    Msg::OpenQuery(super::Evento::Etapa(e))
}

fn escolha_coluna<'a>(colunas: &[String], atual: usize, evento: fn(usize) -> Evento) -> Element<'a, Msg> {
    let opcoes: Vec<String> = colunas.iter().enumerate().map(|(i, n)| format!("{} · {n}", i + 1)).collect();
    let selecionada = opcoes.get(atual).cloned();
    crate::compat::pick_list_vec(opcoes.clone(), selecionada, move |escolhida: String| {
        ev(evento(opcoes.iter().position(|o| *o == escolhida).unwrap_or(0)))
    }).size(TAM_TEXTO).padding([5.0, 8.0]).width(Length::FILL).into()
}

fn campo<'a>(rotulo: &'static str, controle: Element<'a, Msg>) -> Element<'a, Msg> {
    column![texto_oq(rotulo), controle].spacing(ESPACO_PEQUENO).into()
}

fn entrada<'a>(valor: &'a str, dica_campo: &'static str, evento: fn(String) -> Evento) -> Element<'a, Msg> {
    text_input(dica_campo, valor)
        .on_input(move |v| ev(evento(v)))
        .on_submit(ev(Evento::Aplicar))
        .size(TAM_TEXTO).padding([6.0, 8.0])
        .style(estilo_entrada_suspensa).into()
}

/// O cartão do modal (o fundo escurecido é do `view_openquery`).
pub(super) fn painel<'a>(m: &'a Modal, erro: &'a str) -> Element<'a, Msg> {
    let mut corpo = column![titulo_oq(m.titulo())].spacing(ESPACO_GRANDE);
    if m.tipo != "limite" && m.tipo != "agrupar" {
        corpo = corpo.push(campo(tr!("Coluna"), escolha_coluna(&m.colunas, m.col, Evento::Coluna)));
    }
    match m.tipo {
        "filtrar" => {
            corpo = corpo.push(campo(tr!("Manter linhas em que o valor é"), entrada(&m.texto, "Ex.: >100, SP*, <>0", Evento::Texto)))
                .push(legenda_oq(tr!("Aceita =, <>, >, <, >=, <= e * como coringa.")));
        }
        "renomear" => corpo = corpo.push(campo(tr!("Novo nome"), entrada(&m.texto, "Nome", Evento::Texto))),
        "limite" => corpo = corpo.push(campo(tr!("Quantidade de linhas"), entrada(&m.texto, "100", Evento::Texto))),
        "dividir" => {
            corpo = corpo.push(campo(tr!("Delimitador"), entrada(&m.texto, tr!("Espaço, ; , - …"), Evento::Texto)));
            let mut modos = row![].spacing(ESPACO_GRANDE);
            for (rotulo, linhas) in [(tr!("Em colunas"), false), (tr!("Em linhas"), true)] {
                modos = modos.push(pelicano::radio(rotulo, linhas, Some(m.em_linhas), |v| ev(Evento::EmLinhas(v))).size(TAM_TEXTO));
            }
            corpo = corpo.push(campo(tr!("Dividir em"), modos.into()));
        }
        "ordenar" => {
            let mut modos = row![].spacing(ESPACO_GRANDE);
            for (rotulo, desc) in [(tr!("Crescente (A → Z)"), false), (tr!("Decrescente (Z → A)"), true)] {
                modos = modos.push(pelicano::radio(rotulo, desc, Some(m.desc), |v| ev(Evento::Decrescente(v))).size(TAM_TEXTO));
            }
            corpo = corpo.push(campo(tr!("Ordem"), modos.into()));
        }
        "tipar" => {
            let mut modos = row![].spacing(ESPACO_GRANDE);
            for (rotulo, para) in [(tr!("Texto"), "texto"), (tr!("Número"), "numero")] {
                modos = modos.push(pelicano::radio(rotulo, para, Some(m.para), |v| ev(Evento::Para(v))).size(TAM_TEXTO));
            }
            corpo = corpo.push(campo(tr!("Novo tipo"), modos.into()));
        }
        "agrupar" => {
            let mut grupo = column![].spacing(2);
            for (i, nome) in m.colunas.iter().enumerate() {
                grupo = grupo.push(pelicano::checkbox(format!("{} · {nome}", i + 1), m.grupo.contains(&i))
                    .on_toggle(move |_| ev(Evento::Grupo(i))).size(14));
            }
            let nomes: Vec<String> = AGREGACOES.iter().map(|(n, _)| tr!(n).to_string()).collect();
            let atual = nomes.get(m.agregacao).cloned();
            let operacao = crate::compat::pick_list_vec(nomes.clone(), atual, move |escolhida: String| {
                ev(Evento::Agregacao(nomes.iter().position(|o| *o == escolhida).unwrap_or(0)))
            }).size(TAM_TEXTO).padding([5.0, 8.0]).width(Length::FILL);
            corpo = corpo
                .push(campo(tr!("Agrupar por"), container(pelicano::scrollable(grupo)).height(Length::Fixed(150.0)).into()))
                .push(row![
                    container(campo(tr!("Operação"), operacao.into())).width(Length::Fill(1)),
                    container(campo(tr!("Coluna"), escolha_coluna(&m.colunas, m.agregada, Evento::ColunaAgregada))).width(Length::Fill(1)),
                ].spacing(ESPACO_MEDIO))
                .push(campo(tr!("Nome da nova coluna"), entrada(&m.nome_novo, "Quantidade", Evento::NomeNovo)));
        }
        _ => {}
    }
    if !erro.is_empty() {
        corpo = corpo.push(text(erro.to_owned()).size(TAM_TEXTO).color(pelicano::Color::rgb8(202, 85, 85)));
    }
    corpo = corpo.push(row![
        Space::new(Length::FILL, 0.0),
        button(text(tr!("Cancelar")).size(TAM_TEXTO))
            .on_press(ev(Evento::Fechar))
            .style(estilo_botao_secundario).padding([6.0, 12.0]),
        button(text(if m.editar.is_some() { tr!("Salvar etapa") } else { tr!("OK") }).size(TAM_TEXTO))
            .on_press(ev(Evento::Aplicar))
            .style(estilo_botao_primario).padding([6.0, 12.0]),
    ].spacing(ESPACO_MEDIO));
    container(corpo.padding(PADDING_PAINEL + 4.0)).width(Length::Fixed(440.0))
        .style(estilo_janela_oq).into()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn toda_etapa_com_modal_vai_e_volta_pela_engrenagem() {
        let colunas: Vec<String> = ["A", "B", "C"].map(String::from).to_vec();
        for tipo in COM_MODAL {
            let mut m = Modal::novo(tipo, colunas.clone(), 1, &[0, 2]);
            m.atualizar(Evento::Texto(";".into()));
            if tipo == "limite" { m.atualizar(Evento::Texto("5".into())); }
            let passo = m.passo().unwrap_or_else(|e| panic!("{tipo}: {e}"));
            let volta = Modal::de_passo(&passo, 3, colunas.clone()).unwrap();
            assert_eq!((volta.tipo, volta.editar), (tipo, Some(3)), "{tipo}");
            assert_eq!(format!("{:?}", volta.passo().unwrap()), format!("{passo:?}"), "{tipo}");
        }
    }

    #[test]
    fn dividir_vai_para_colunas_por_padrao() {
        let m = Modal::novo("dividir", vec!["A".into()], 0, &[]);
        assert!(matches!(m.passo(), Ok(Passo::DividirColunas { .. })));
    }
}
