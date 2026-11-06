//! Modal de uma etapa com parâmetros (filtrar, renomear, manter/remover linhas,
//! dividir, ordenar, tipo, agrupar, formatar, preencher, substituir, coluna
//! condicional, extrair da data, coluna personalizada): cria a etapa pela faixa
//! ou pelo menu da coluna, e edita uma etapa já aplicada pela engrenagem, como no
//! Power Query.
use super::*;
use pelicano::{column, row};

/// Etapas que têm modal (as outras são aplicadas direto).
pub(super) const COM_MODAL: [&str; 15] = ["filtrar", "renomear", "limite", "dividir", "ordenar", "tipar", "agrupar",
    "formatar", "preencher", "remover_linhas", "substituir", "substituir_erros", "condicional", "derivar", "personalizada"];

const AGREGACOES: [(&str, Agregacao); 8] = [
    ("Contar linhas", Agregacao::ContarValores),
    ("Soma", Agregacao::Soma),
    ("Média", Agregacao::Media),
    ("Mínimo", Agregacao::Minimo),
    ("Máximo", Agregacao::Maximo),
    ("Mediana", Agregacao::Mediana),
    ("Contar distintos", Agregacao::Distintos),
    ("Contar números", Agregacao::Contagem),
];

/// (valor no passo, rótulo) das opções de cada tipo.
const FORMATOS: [(&str, &str); 7] = [("aparar", "Aparar espaços"), ("limpar", "Limpar caracteres invisíveis"),
    ("maiusculas", "MAIÚSCULAS"), ("minusculas", "minúsculas"), ("titulo", "Primeira Maiúscula"),
    ("prefixo", "Acrescentar prefixo"), ("sufixo", "Acrescentar sufixo")];
const REMOCOES: [(&str, &str); 4] = [("primeiras", "Remover as primeiras"), ("ultimas", "Remover as últimas"),
    ("manter_ultimas", "Manter só as últimas"), ("vazias", "Remover linhas em branco")];
const DIVISOES: [(&str, &str); 3] = [("delimitador", "Por delimitador"), ("posicoes", "Por posições"), ("cada", "A cada N caracteres")];
const OPERADORES: [(&str, &str); 7] = [("=", "é igual a"), ("!=", "é diferente de"), ("contem", "contém"),
    ("comeca_com", "começa com"), ("termina_com", "termina com"), (">", "é maior que"), ("<", "é menor que")];
const PARTES_DATA: [(&str, &str); 6] = [("ano", "Ano"), ("trimestre", "Trimestre"), ("mes", "Nome do mês"),
    ("anoMes", "Ano-mês"), ("dia", "Dia"), ("diaSemana", "Dia da semana")];

#[derive(Debug, Clone)]
pub enum Evento {
    Texto(String),
    Texto2(String),
    Texto3(String),
    Coluna(usize),
    EmLinhas(bool),
    Decrescente(bool),
    Para(&'static str),
    /// A opção do tipo (como formatar, que linhas remover, operador…).
    Opcao(&'static str),
    /// Liga/desliga uma coluna do "Agrupar por".
    Grupo(usize),
    /// Conta `i` do agrupar: operação, coluna e nome.
    Agregacao(usize, usize),
    ColunaAgregada(usize, usize),
    NomeNovo(usize, String),
    MaisAgregacao,
    MenosAgregacao(usize),
    Aplicar,
    Fechar,
}

/// Uma conta do "Agrupar por" (índice em AGREGACOES, coluna, nome da nova coluna).
#[derive(Debug, Clone)]
struct Conta { operacao: usize, col: usize, nome: String }

pub(super) struct Modal {
    pub tipo: &'static str,
    /// `Some(i)`: edita a etapa `i` em vez de acrescentar.
    pub editar: Option<usize>,
    /// Colunas que chegam a esta etapa.
    colunas: Vec<String>,
    col: usize,
    /// Filtro, novo nome, quantidade, separador, valor procurado, nome da coluna nova…
    texto: String,
    /// Segundo e terceiro campo (substituir por, então, senão, fórmula…).
    texto2: String,
    texto3: String,
    em_linhas: bool,
    desc: bool,
    para: &'static str,
    opcao: &'static str,
    grupo: Vec<usize>,
    contas: Vec<Conta>,
}

/// Valor digitado num campo: vazio é vazio, número em pt-BR vira número, o resto é texto.
fn valor_digitado(s: &str) -> Value {
    let t = s.trim();
    if t.is_empty() { return Value::Empty; }
    match feathersheet_core::celulas::number::from_js_string(&t.replace(',', ".")) {
        Some(n) if !t.contains(' ') => Value::Number(n),
        _ => Value::Text(s.to_owned()),
    }
}

fn valor_para_campo(v: &Value) -> String {
    match v {
        Value::Number(n) => feathersheet_core::celulas::number::to_js_string(*n).replace('.', ","),
        outro => feathersheet_core::celulas::values::to_text(outro),
    }
}

fn opcao_de(lista: &[(&'static str, &str)], valor: &str) -> &'static str {
    lista.iter().find(|(v, _)| *v == valor).map(|(v, _)| *v).unwrap_or(lista[0].0)
}

impl Modal {
    /// Etapa nova sobre `colunas` (o resultado atual), com `selecionadas` em destaque.
    pub fn novo(tipo: &'static str, colunas: Vec<String>, col: usize, selecionadas: &[usize]) -> Self {
        let nome_col = colunas.get(col).cloned().unwrap_or_default();
        let (texto, texto2, texto3, opcao) = match tipo {
            "renomear" => (nome_col.clone(), String::new(), String::new(), ""),
            "limite" => ("100".into(), String::new(), String::new(), ""),
            "remover_linhas" => ("1".into(), String::new(), String::new(), "primeiras"),
            "dividir" => (" ".into(), "0, 2".into(), "3".into(), "delimitador"),
            "formatar" => (String::new(), String::new(), String::new(), "aparar"),
            "preencher" => (String::new(), String::new(), String::new(), "baixo"),
            "substituir_erros" => ("0".into(), String::new(), String::new(), ""),
            "condicional" => (tr!("Personalizada").into(), String::new(), String::new(), "="),
            "derivar" => (String::new(), String::new(), String::new(), "ano"),
            "personalizada" => (tr!("Personalizada").into(), format!("=[{nome_col}]"), String::new(), ""),
            _ => (String::new(), String::new(), String::new(), ""),
        };
        let grupo = if selecionadas.is_empty() { vec![col] } else { selecionadas.to_vec() };
        Self {
            tipo, editar: None, colunas, col, texto, texto2, texto3, em_linhas: false, desc: false, para: "texto",
            opcao, grupo, contas: vec![Conta { operacao: 0, col, nome: tr!("Quantidade").into() }],
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
            Passo::RemoverLinhas { modo, quantas } => {
                m.tipo = "remover_linhas";
                m.opcao = opcao_de(&REMOCOES, modo);
                m.texto = quantas.to_string();
            }
            Passo::Dividir { col, separador } | Passo::DividirColunas { col, separador } => {
                m.tipo = "dividir";
                m.col = *col;
                m.texto = separador.clone();
                m.em_linhas = matches!(passo, Passo::Dividir { .. });
            }
            Passo::DividirPosicoes { col, posicoes, cada } => {
                m.tipo = "dividir";
                m.col = *col;
                match cada {
                    Some(n) => { m.opcao = "cada"; m.texto3 = n.to_string(); }
                    None => {
                        m.opcao = "posicoes";
                        m.texto2 = posicoes.iter().map(usize::to_string).collect::<Vec<_>>().join(", ");
                    }
                }
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
                m.contas = agregar.iter().map(|a| Conta {
                    operacao: AGREGACOES.iter().position(|(_, g)| *g == a.como).unwrap_or(0),
                    col: a.col,
                    nome: a.nome.clone().unwrap_or_default(),
                }).collect();
                if m.contas.is_empty() { return None; }
            }
            Passo::Formatar { col, como, texto } => {
                m.tipo = "formatar";
                m.col = *col;
                m.opcao = opcao_de(&FORMATOS, como);
                m.texto = texto.clone().unwrap_or_default();
            }
            Passo::Preencher { col, para_cima } => {
                m.tipo = "preencher";
                m.col = *col;
                m.opcao = if *para_cima { "cima" } else { "baixo" };
            }
            Passo::Substituir { col, de, para } => {
                m.tipo = "substituir";
                m.col = *col;
                m.texto = valor_para_campo(de);
                m.texto2 = valor_para_campo(para);
            }
            Passo::SubstituirErros { col, para } => {
                m.tipo = "substituir_erros";
                m.col = *col;
                m.texto = valor_para_campo(para);
            }
            Passo::Condicional { nome, col, operador, valor, entao, senao } => {
                m.tipo = "condicional";
                m.col = *col;
                m.opcao = opcao_de(&OPERADORES, operador);
                m.texto = nome.clone();
                m.texto2 = valor.clone();
                m.texto3 = format!("{entao}\u{1}{senao}");
            }
            Passo::Derivar { col, parte, nome } => {
                m.tipo = "derivar";
                m.col = *col;
                m.opcao = opcao_de(&PARTES_DATA, parte);
                m.texto = nome.clone().unwrap_or_default();
            }
            Passo::Personalizada { nome, formula } => {
                m.tipo = "personalizada";
                m.texto = nome.clone();
                m.texto2 = formula.clone();
            }
            _ => return None,
        }
        Some(m)
    }

    /// Então/senão da coluna condicional moram em `texto3`, separados por \u{1}.
    fn entao_senao(&self) -> (String, String) {
        let (a, b) = self.texto3.split_once('\u{1}').unwrap_or((self.texto3.as_str(), ""));
        (a.to_owned(), b.to_owned())
    }

    pub fn atualizar(&mut self, ev: Evento) {
        match ev {
            Evento::Texto(t) => self.texto = t,
            Evento::Texto2(t) => self.texto2 = t,
            Evento::Texto3(t) => {
                // Na condicional, Texto3 é o "então" (o "senão" vem por `definir_senao`).
                if self.tipo == "condicional" {
                    let (_, senao) = self.entao_senao();
                    self.texto3 = format!("{t}\u{1}{senao}");
                } else {
                    self.texto3 = t;
                }
            }
            Evento::Coluna(c) => {
                if self.tipo == "renomear" && self.colunas.get(self.col) == Some(&self.texto) {
                    self.texto = self.colunas.get(c).cloned().unwrap_or_default();
                }
                self.col = c;
            }
            Evento::EmLinhas(v) => self.em_linhas = v,
            Evento::Decrescente(v) => self.desc = v,
            Evento::Para(p) => self.para = p,
            Evento::Opcao(o) => self.opcao = o,
            Evento::Grupo(c) => match self.grupo.iter().position(|g| *g == c) {
                Some(i) => { self.grupo.remove(i); }
                None => { self.grupo.push(c); self.grupo.sort_unstable(); }
            },
            Evento::Agregacao(i, op) => if let Some(c) = self.contas.get_mut(i) { c.operacao = op },
            Evento::ColunaAgregada(i, col) => if let Some(c) = self.contas.get_mut(i) { c.col = col },
            Evento::NomeNovo(i, n) => if let Some(c) = self.contas.get_mut(i) { c.nome = n },
            Evento::MaisAgregacao => {
                let col = self.contas.last().map_or(self.col, |c| c.col);
                self.contas.push(Conta { operacao: 1, col, nome: String::new() });
            }
            Evento::MenosAgregacao(i) => if self.contas.len() > 1 && i < self.contas.len() { self.contas.remove(i); },
            Evento::Aplicar | Evento::Fechar => {}
        }
    }

    fn senao(&mut self, t: String) {
        let (entao, _) = self.entao_senao();
        self.texto3 = format!("{entao}\u{1}{t}");
    }

    pub fn passo(&self) -> Result<Passo, String> {
        let col = self.col;
        let quantidade = |t: &str| -> Result<f64, String> {
            let n = t.trim().replace(',', ".").parse::<f64>()
                .map_err(|_| tr!("Quantidade de linhas inválida").to_string())?;
            if !n.is_finite() || n < 0.0 { return Err(tr!("Informe um limite não negativo e finito").into()); }
            Ok(n)
        };
        Ok(match self.tipo {
            "filtrar" => Passo::Filtrar { col, criterio: Value::Text(self.texto.clone()) },
            "renomear" => {
                if self.texto.trim().is_empty() {
                    return Err(tr!("Informe o novo nome da coluna").into());
                }
                Passo::Renomear { col, nome: self.texto.trim().into() }
            }
            "limite" => Passo::Limite { quantas: quantidade(&self.texto)? },
            "remover_linhas" => Passo::RemoverLinhas {
                modo: self.opcao.into(),
                quantas: if self.opcao == "vazias" { 0.0 } else { quantidade(&self.texto)? },
            },
            "dividir" => match self.opcao {
                "posicoes" => {
                    let posicoes = self.texto2.split([',', ';', ' ']).filter(|p| !p.trim().is_empty())
                        .map(|p| p.trim().parse::<usize>()).collect::<Result<Vec<_>, _>>()
                        .map_err(|_| tr!("Posições: números separados por vírgula (ex.: 0, 2, 6)").to_string())?;
                    if posicoes.is_empty() { return Err(tr!("Posições: números separados por vírgula (ex.: 0, 2, 6)").into()); }
                    Passo::DividirPosicoes { col, posicoes, cada: None }
                }
                "cada" => {
                    let n = self.texto3.trim().parse::<usize>().ok().filter(|n| *n > 0)
                        .ok_or_else(|| tr!("Informe de quantos em quantos caracteres dividir").to_string())?;
                    Passo::DividirPosicoes { col, posicoes: Vec::new(), cada: Some(n) }
                }
                _ => {
                    if self.texto.is_empty() {
                        return Err(tr!("Informe um separador antes de dividir").into());
                    }
                    let separador = self.texto.clone();
                    if self.em_linhas { Passo::Dividir { col, separador } } else { Passo::DividirColunas { col, separador } }
                }
            },
            "ordenar" => Passo::Ordenar { por: vec![ChaveOrdenacao { col, desc: Some(self.desc) }] },
            "tipar" => Passo::Tipar { col, para: self.para.into() },
            "agrupar" => {
                if self.grupo.is_empty() {
                    return Err(tr!("Escolha ao menos uma coluna em Agrupar por").into());
                }
                Passo::Agrupar {
                    por: self.grupo.clone(),
                    agregar: self.contas.iter().map(|c| Agregado {
                        col: c.col,
                        como: AGREGACOES[c.operacao].1,
                        nome: Some(c.nome.trim().to_owned()).filter(|n| !n.is_empty()),
                    }).collect(),
                }
            }
            "formatar" => Passo::Formatar {
                col, como: self.opcao.into(),
                texto: matches!(self.opcao, "prefixo" | "sufixo").then(|| self.texto.clone()),
            },
            "preencher" => Passo::Preencher { col, para_cima: self.opcao == "cima" },
            "substituir" => Passo::Substituir { col, de: valor_digitado(&self.texto), para: valor_digitado(&self.texto2) },
            "substituir_erros" => Passo::SubstituirErros { col, para: valor_digitado(&self.texto) },
            "condicional" => {
                if self.texto.trim().is_empty() { return Err(tr!("Informe o nome da nova coluna").into()); }
                let (entao, senao) = self.entao_senao();
                Passo::Condicional { nome: self.texto.trim().into(), col, operador: self.opcao.into(),
                    valor: self.texto2.clone(), entao, senao }
            }
            "derivar" => Passo::Derivar { col, parte: self.opcao.into(),
                nome: Some(self.texto.trim().to_owned()).filter(|n| !n.is_empty()) },
            "personalizada" => {
                if self.texto.trim().is_empty() { return Err(tr!("Informe o nome da nova coluna").into()); }
                feathersheet_core::consulta::query::traduzir_formula(&self.texto2, &self.colunas)?;
                Passo::Personalizada { nome: self.texto.trim().into(), formula: self.texto2.trim().into() }
            }
            _ => return Err(tr!("Etapa sem parâmetros").into()),
        })
    }

    fn titulo(&self) -> &'static str {
        match self.tipo {
            "filtrar" => tr!("Filtrar linhas"),
            "renomear" => tr!("Renomear coluna"),
            "limite" => tr!("Manter primeiras linhas"),
            "remover_linhas" => tr!("Remover linhas"),
            "dividir" => tr!("Dividir coluna"),
            "ordenar" => tr!("Classificar"),
            "tipar" => tr!("Alterar tipo"),
            "formatar" => tr!("Formatar texto"),
            "preencher" => tr!("Preencher"),
            "substituir" => tr!("Substituir valores"),
            "substituir_erros" => tr!("Substituir erros"),
            "condicional" => tr!("Coluna condicional"),
            "derivar" => tr!("Extrair da data"),
            "personalizada" => tr!("Coluna personalizada"),
            _ => tr!("Agrupar por"),
        }
    }
}

fn ev(e: Evento) -> Msg {
    Msg::OpenQuery(super::Evento::Etapa(e))
}

fn lista<'a>(opcoes: Vec<String>, atual: usize, f: impl Fn(usize) -> Msg + 'static) -> Element<'a, Msg> {
    let selecionada = opcoes.get(atual).cloned();
    let copia = opcoes.clone();
    crate::compat::pick_list_vec(opcoes, selecionada, move |escolhida: String| {
        f(copia.iter().position(|o| *o == escolhida).unwrap_or(0))
    }).size(TAM_TEXTO).padding([5.0, 8.0]).width(Length::FILL).into()
}

fn escolha_coluna<'a>(colunas: &[String], atual: usize, evento: impl Fn(usize) -> Evento + 'static) -> Element<'a, Msg> {
    let opcoes: Vec<String> = colunas.iter().enumerate().map(|(i, n)| format!("{} · {n}", i + 1)).collect();
    lista(opcoes, atual, move |i| ev(evento(i)))
}

/// Lista suspensa das opções (`valor`, rótulo) de um tipo.
fn escolha_opcao<'a>(opcoes: &[(&'static str, &'static str)], atual: &str) -> Element<'a, Msg> {
    let rotulos: Vec<String> = opcoes.iter().map(|(_, r)| tr!(*r).to_string()).collect();
    let valores: Vec<&'static str> = opcoes.iter().map(|(v, _)| *v).collect();
    let i = valores.iter().position(|v| *v == atual).unwrap_or(0);
    lista(rotulos, i, move |i| ev(Evento::Opcao(valores[i])))
}

fn campo<'a>(rotulo: &'static str, controle: Element<'a, Msg>) -> Element<'a, Msg> {
    column![texto_oq(rotulo), controle].spacing(ESPACO_PEQUENO).into()
}

fn entrada<'a>(valor: &'a str, dica_campo: &'static str, evento: impl Fn(String) -> Evento + 'static) -> Element<'a, Msg> {
    text_input(dica_campo, valor)
        .on_input(move |v| ev(evento(v)))
        .on_submit(ev(Evento::Aplicar))
        .size(TAM_TEXTO).padding([6.0, 8.0])
        .style(estilo_entrada_suspensa).into()
}

fn radios<'a>(opcoes: &[(&'static str, &'static str)], atual: &'static str) -> Element<'a, Msg> {
    let mut modos = row![].spacing(ESPACO_GRANDE);
    for (valor, rotulo) in opcoes {
        modos = modos.push(pelicano::radio(tr!(*rotulo), *valor, Some(atual), |v| ev(Evento::Opcao(v))).size(TAM_TEXTO));
    }
    modos.into()
}

/// O cartão do modal (o fundo escurecido é do `view_openquery`).
pub(super) fn painel<'a>(m: &'a Modal, erro: &'a str) -> Element<'a, Msg> {
    let mut corpo = column![titulo_oq(m.titulo())].spacing(ESPACO_GRANDE);
    if !matches!(m.tipo, "limite" | "agrupar" | "remover_linhas" | "personalizada") {
        corpo = corpo.push(campo(tr!("Coluna"), escolha_coluna(&m.colunas, m.col, Evento::Coluna)));
    }
    match m.tipo {
        "filtrar" => {
            corpo = corpo.push(campo(tr!("Manter linhas em que o valor é"), entrada(&m.texto, "Ex.: >100, SP*, <>0", Evento::Texto)))
                .push(legenda_oq(tr!("Aceita =, <>, >, <, >=, <= e * como coringa.")));
        }
        "renomear" => corpo = corpo.push(campo(tr!("Novo nome"), entrada(&m.texto, "Nome", Evento::Texto))),
        "limite" => corpo = corpo.push(campo(tr!("Quantidade de linhas"), entrada(&m.texto, "100", Evento::Texto))),
        "remover_linhas" => {
            corpo = corpo.push(campo(tr!("O que fazer"), escolha_opcao(&REMOCOES, m.opcao)));
            if m.opcao != "vazias" {
                corpo = corpo.push(campo(tr!("Quantidade de linhas"), entrada(&m.texto, "1", Evento::Texto)));
            } else {
                corpo = corpo.push(legenda_oq(tr!("Sai a linha em que todas as colunas estão vazias.")));
            }
        }
        "dividir" => {
            corpo = corpo.push(campo(tr!("Como dividir"), escolha_opcao(&DIVISOES, m.opcao)));
            match m.opcao {
                "posicoes" => {
                    corpo = corpo.push(campo(tr!("Posições"), entrada(&m.texto2, "0, 2, 6", Evento::Texto2)))
                        .push(legenda_oq(tr!("Onde cada pedaço começa, contando os caracteres a partir de 0: \"SP2026001\" com 0, 2, 6 vira SP | 2026 | 001.")));
                }
                "cada" => corpo = corpo.push(campo(tr!("Quantidade de caracteres"), entrada(&m.texto3, "3", Evento::Texto3))),
                _ => {
                    corpo = corpo.push(campo(tr!("Delimitador"), entrada(&m.texto, tr!("Espaço, ; , - …"), Evento::Texto)));
                    let mut modos = row![].spacing(ESPACO_GRANDE);
                    for (rotulo, linhas) in [(tr!("Em colunas"), false), (tr!("Em linhas"), true)] {
                        modos = modos.push(pelicano::radio(rotulo, linhas, Some(m.em_linhas), |v| ev(Evento::EmLinhas(v))).size(TAM_TEXTO));
                    }
                    corpo = corpo.push(campo(tr!("Dividir em"), modos.into()));
                }
            }
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
            corpo = corpo.push(campo(tr!("Agrupar por"), container(pelicano::scrollable(grupo)).height(Length::Fixed(130.0)).into()));
            let nomes: Vec<String> = AGREGACOES.iter().map(|(n, _)| tr!(*n).to_string()).collect();
            let mut contas = column![row![
                container(texto_oq(tr!("Nome da nova coluna"))).width(Length::Fill(1)),
                container(texto_oq(tr!("Operação"))).width(Length::Fill(1)),
                container(texto_oq(tr!("Coluna"))).width(Length::Fill(1)),
                Space::new(Length::Fixed(28.0), 0.0),
            ].spacing(ESPACO_PEQUENO)].spacing(ESPACO_PEQUENO);
            for (i, c) in m.contas.iter().enumerate() {
                contas = contas.push(row![
                    container(entrada(&c.nome, "Total", move |t| Evento::NomeNovo(i, t))).width(Length::Fill(1)),
                    container(lista(nomes.clone(), c.operacao, move |op| ev(Evento::Agregacao(i, op)))).width(Length::Fill(1)),
                    container(escolha_coluna(&m.colunas, c.col, move |col| Evento::ColunaAgregada(i, col))).width(Length::Fill(1)),
                    button(text("−").size(TAM_TEXTO)).padding([4.0, 9.0]).style(estilo_botao_secundario)
                        .on_press_maybe((m.contas.len() > 1).then(|| ev(Evento::MenosAgregacao(i)))),
                ].spacing(ESPACO_PEQUENO).align(pelicano::Align::Center));
            }
            corpo = corpo.push(contas).push(button(text(tr!("+ Adicionar conta")).size(TAM_TEXTO))
                .on_press(ev(Evento::MaisAgregacao)).style(estilo_botao_secundario).padding([5.0, 10.0]));
        }
        "formatar" => {
            corpo = corpo.push(campo(tr!("Formatar como"), escolha_opcao(&FORMATOS, m.opcao)));
            if matches!(m.opcao, "prefixo" | "sufixo") {
                corpo = corpo.push(campo(tr!("Texto a acrescentar"), entrada(&m.texto, "UF-", Evento::Texto)));
            }
        }
        "preencher" => {
            corpo = corpo.push(campo(tr!("Direção"), radios(&[("baixo", "Para baixo"), ("cima", "Para cima")], m.opcao)))
                .push(legenda_oq(tr!("As células vazias recebem o valor da célula de cima (ou de baixo).")));
        }
        "substituir" => {
            corpo = corpo.push(campo(tr!("Valor a procurar"), entrada(&m.texto, tr!("vazio = células em branco"), Evento::Texto)))
                .push(campo(tr!("Substituir por"), entrada(&m.texto2, "0", Evento::Texto2)))
                .push(legenda_oq(tr!("A célula inteira precisa ser igual ao valor procurado. Número com vírgula vira número.")));
        }
        "substituir_erros" => {
            corpo = corpo.push(campo(tr!("Substituir os erros por"), entrada(&m.texto, "0", Evento::Texto)));
        }
        "condicional" => {
            let (entao, senao) = m.entao_senao();
            corpo = corpo
                .push(campo(tr!("Nome da nova coluna"), entrada(&m.texto, "Status", Evento::Texto)))
                .push(row![
                    container(campo(tr!("Se o valor"), escolha_opcao(&OPERADORES, m.opcao))).width(Length::Fill(1)),
                    container(campo(tr!("Valor"), entrada(&m.texto2, "1000", Evento::Texto2))).width(Length::Fill(1)),
                ].spacing(ESPACO_MEDIO))
                .push(row![
                    container(campo(tr!("Então"), text_input("Alto", entao).on_input(|v| ev(Evento::Texto3(v)))
                        .size(TAM_TEXTO).padding([6.0, 8.0]).style(estilo_entrada_suspensa).into())).width(Length::Fill(1)),
                    container(campo(tr!("Senão"), text_input("Baixo", senao).on_input(|v| Msg::OpenQuery(super::Evento::EtapaSenao(v)))
                        .size(TAM_TEXTO).padding([6.0, 8.0]).style(estilo_entrada_suspensa).into())).width(Length::Fill(1)),
                ].spacing(ESPACO_MEDIO));
        }
        "derivar" => {
            corpo = corpo.push(campo(tr!("Parte da data"), escolha_opcao(&PARTES_DATA, m.opcao)))
                .push(campo(tr!("Nome da nova coluna (opcional)"), entrada(&m.texto, "Ano", Evento::Texto)));
        }
        "personalizada" => {
            corpo = corpo
                .push(campo(tr!("Nome da nova coluna"), entrada(&m.texto, "Total", Evento::Texto)))
                .push(campo(tr!("Fórmula"), entrada(&m.texto2, "=[Preço]*[Qtd]", Evento::Texto2)))
                .push(legenda_oq(tr!("Fórmula da planilha, com [Coluna] para o valor da linha: =[Preço]*[Qtd], =SE([Valor]>1000;\"Alto\";\"Baixo\").")))
                .push(legenda_oq(trf!("Colunas: {}", m.colunas.iter().map(|c| format!("[{c}]")).collect::<Vec<_>>().join(" "))));
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
    let largura = if m.tipo == "agrupar" { 560.0 } else { 440.0 };
    container(corpo.padding(PADDING_PAINEL + 4.0)).width(Length::Fixed(largura))
        .style(estilo_janela_oq).into()
}

impl Modal {
    /// O "senão" da coluna condicional (campo próprio, fora do `Evento`).
    pub(super) fn definir_senao(&mut self, t: String) { self.senao(t); }
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
            if matches!(tipo, "limite" | "remover_linhas") { m.atualizar(Evento::Texto("5".into())); }
            if matches!(tipo, "condicional" | "personalizada") { m.atualizar(Evento::Texto("Nova".into())); }
            let passo = m.passo().unwrap_or_else(|e| panic!("{tipo}: {e}"));
            let volta = Modal::de_passo(&passo, 3, colunas.clone()).unwrap_or_else(|| panic!("{tipo} sem volta"));
            assert_eq!((volta.tipo, volta.editar), (tipo, Some(3)), "{tipo}");
            assert_eq!(format!("{:?}", volta.passo().unwrap()), format!("{passo:?}"), "{tipo}");
        }
    }

    #[test]
    fn opcoes_dos_modais_viram_a_etapa_certa() {
        let colunas: Vec<String> = ["Cod", "Valor"].map(String::from).to_vec();
        let mut m = Modal::novo("dividir", colunas.clone(), 0, &[]);
        assert!(matches!(m.passo(), Ok(Passo::DividirColunas { .. })), "padrão: por delimitador, em colunas");
        m.atualizar(Evento::Opcao("posicoes"));
        m.atualizar(Evento::Texto2("0, 2, 6".into()));
        assert!(matches!(m.passo(), Ok(Passo::DividirPosicoes { posicoes, cada: None, .. }) if posicoes == vec![0, 2, 6]));
        let mut m = Modal::novo("agrupar", colunas.clone(), 0, &[]);
        m.atualizar(Evento::MaisAgregacao);
        m.atualizar(Evento::ColunaAgregada(1, 1));
        m.atualizar(Evento::NomeNovo(1, "Total".into()));
        let Ok(Passo::Agrupar { agregar, .. }) = m.passo() else { panic!() };
        assert_eq!(agregar.len(), 2);
        assert_eq!((agregar[1].col, agregar[1].como, agregar[1].nome.as_deref()), (1, Agregacao::Soma, Some("Total")));
        let mut m = Modal::novo("substituir", colunas.clone(), 1, &[]);
        m.atualizar(Evento::Texto("N/D".into()));
        m.atualizar(Evento::Texto2("1,5".into()));
        assert!(matches!(m.passo(), Ok(Passo::Substituir { de: Value::Text(d), para: Value::Number(p), .. }) if d == "N/D" && p == 1.5));
        let mut m = Modal::novo("condicional", colunas.clone(), 1, &[]);
        m.atualizar(Evento::Opcao(">"));
        m.atualizar(Evento::Texto2("1000".into()));
        m.atualizar(Evento::Texto3("Alto".into()));
        m.definir_senao("Baixo".into());
        assert!(matches!(m.passo(), Ok(Passo::Condicional { entao, senao, operador, .. }) if entao == "Alto" && senao == "Baixo" && operador == ">"));
        let mut m = Modal::novo("personalizada", colunas, 0, &[]);
        m.atualizar(Evento::Texto2("=[Nada]*2".into()));
        assert!(m.passo().unwrap_err().contains("Nada"), "coluna que não existe é avisada no modal");
    }
}
